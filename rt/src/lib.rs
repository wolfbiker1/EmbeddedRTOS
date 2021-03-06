//!
//! Contains the first entrypoint for startup procedure.
//! Only for runtime purposes, acts as a platform for the kernel
//! code build on top of it.
//!
#![no_main]
#![no_std]
#![feature(asm)]

pub mod dev;
pub mod generic;
pub mod interrupts;
pub mod mem;
pub mod sched;
pub mod sys;

use crate::sched::{task_control};
use core::panic::PanicInfo;
use core::ptr;
use dev::{tim3, uart::print_from_ptr};
use generic::platform::{self, adresses, offsets, bitfields};
use generic::cpu::{self, c_adresses, c_offsets, c_bitfields};
use interrupts::systick::{disable_systick, enable_systick};
use mem::memory_handler::{read, write};
use sched::scheduler::{save_task_context, load_next_task, load_sleeping_task};
use sys::call_api::TrapMeta;

fn enable_gpio_e_leds() {
    // see p 54 reg boundaries
    let gpio_port_e11 = dev::gpio_driver::GpioX::new("E", 11);
    gpio_port_e11.set_moder(dev::gpio_types::ModerTypes::GeneralPurposeOutputMode);
    gpio_port_e11.set_otyper(dev::gpio_types::OutputTypes::PushPull);
    let gpio_port_e14 = dev::gpio_driver::GpioX::new("E", 12);
    gpio_port_e14.set_moder(dev::gpio_types::ModerTypes::GeneralPurposeOutputMode);
    gpio_port_e14.set_otyper(dev::gpio_types::OutputTypes::PushPull);

    let gpio_port_e14 = dev::gpio_driver::GpioX::new("E", 14);
    gpio_port_e14.set_moder(dev::gpio_types::ModerTypes::GeneralPurposeOutputMode);
    gpio_port_e14.set_otyper(dev::gpio_types::OutputTypes::PushPull);

    let gpio_port_e14 = dev::gpio_driver::GpioX::new("E", 13);
    gpio_port_e14.set_moder(dev::gpio_types::ModerTypes::GeneralPurposeOutputMode);
    gpio_port_e14.set_otyper(dev::gpio_types::OutputTypes::PushPull);
}

fn setup_clock_system() {
    // turn on gpio clock
    // see p 166 -> IOPAEN
    let rcc_ahbenr = adresses::RCC | offsets::rcc::RCC_AHBENR;
    write(rcc_ahbenr, 1 << bitfields::rcc::IOPAEN | 1 << bitfields::rcc::IOPEEN);

    // TIM2 and 3 EN -> p 166
    let rcc_apb1enr: u32 = adresses::RCC | offsets::rcc::RCC_APB1ENR;
    write(rcc_apb1enr, read(rcc_apb1enr) | 0b11);

    // USART1EN -> p 166
    let rcc_apb2enr: u32 = adresses::RCC | offsets::rcc::RCC_APB2ENR;
    write(rcc_apb2enr, read(rcc_apb2enr) | ((0b1 << bitfields::rcc::USART1EN) | bitfields::rcc::SYSCFGEN));
}

fn enable_serial_printing() {
    let gpio_port_a0 = dev::gpio_driver::GpioX::new("A", 9);
    gpio_port_a0.set_moder(dev::gpio_types::ModerTypes::AlternateFunctionMode);
    gpio_port_a0.set_otyper(dev::gpio_types::OutputTypes::PushPull);
    gpio_port_a0.into_af(7);

    let usart1 = dev::uart::new(1, 9600);
    usart1.enable();
}

///
/// Mandatory resetfunction at adress 0x08000004.
/// Gets called after power on the cpu.
///
#[no_mangle]
pub unsafe extern "C" fn Reset() -> ! {
    setup_clock_system();
    enable_gpio_e_leds();
    enable_serial_printing();

    tim3::set_prescaler(1000);
    tim3::set_ug();
    tim3::clear_uif();
    tim3::clear_udis();
    tim3::enable_interrupt();

    // interrupts::systick::STK::set_up_systick(30);

    dev::uart::print_str("#########################\n\r");
    dev::uart::print_str("# WELCOME TO STM32 RTOS #\n\r");
    dev::uart::print_str("#########################\n\r");

    // dev::uart::print_dec(123);

    extern "C" {
        static mut _sbss: u8;
        static mut _ebss: u8;

        static mut _sdata: u8;
        static mut _edata: u8;
        static _sidata: u8;
    }

    let count = &_ebss as *const u8 as usize - &_sbss as *const u8 as usize;
    ptr::write_bytes(&mut _sbss as *mut u8, 0, count);

    let count = &_edata as *const u8 as usize - &_sdata as *const u8 as usize;
    ptr::copy_nonoverlapping(&_sidata as *const u8, &mut _sdata as *mut u8, count);

    let nvic_iser: u32 = 0xE000E100;
    let existing_value = ptr::read_volatile(nvic_iser as *mut u32);
    ptr::write_volatile(nvic_iser as *mut u32, existing_value | 0b1 << 29);

    extern "Rust" {
        fn main() -> !;
    }

    main()
}

#[panic_handler]
fn panic(_panic: &PanicInfo<'_>) -> ! {
    loop {}
}

pub union Vector {
    reserved: u32,
    handler: unsafe extern "C" fn(),
}

pub union VectorDivergentFn {
    _reserved: u32,
    handler: unsafe extern "C" fn() -> !,
}

extern "C" {
    fn NMI();
    fn HardFault();
    fn MemManage();
    fn BusFault();
    fn UsageFault();
    fn __get_r0() -> u32;
    fn __return_to_user_mode();
}

#[no_mangle]
pub extern "C" fn SysTick() {
    disable_systick();
    set_pending();
}

fn set_pending() {
    // Interrupt control and state register, page 225
    // baseadress: scb, p221 4.4, line 3
    // offset: 4.4.3, p225
    let icsr_pendsvset: u32 = c_adresses::SCB | c_offsets::scb::ICSR;
    let existing_value = read(icsr_pendsvset);
    write(icsr_pendsvset, existing_value | (0b1 << c_bitfields::icsr::PENDSVSET));
}

///
///
///
///
///
///
#[no_mangle]
pub extern "C" fn PendSV() {
    // sched::scheduler::context_switch();
    save_task_context();
    load_next_task();
    enable_systick();
    unsafe {
        __return_to_user_mode();
    }
}

///
/// Handles predefined trap instructions.
/// A trap may only generated from a function with
/// is provided from the syscall api. Bevor
/// executing the trap instruction, the according
/// syscall id gets written into register r2, which
/// then gets matched within the handler to provide
/// the requested service to the user.
///
#[no_mangle]
pub extern "C" fn SVCall() {
    unsafe {
        let trap_meta_info: &mut TrapMeta = &mut *(__get_r0() as *mut TrapMeta);
        match trap_meta_info.id {
            // the calling task passes its desired sleep value within
            // the trap id. in according to that value the capture compare register of
            // timer 3 gets configured. the task state is set to sleeping, the
            // timer gets startet and when finishing this steps the scheduler
            // gets triggered so load the next running task.
            sys::call_api::TrapReason::Sleep => {
                let time_to_sleep = trap_meta_info.payload;
                tim3::set_ccr((*time_to_sleep * 8) as u16);
                tim3::set_ug();
                task_control::mark_self_as_sleeping();

                tim3::start();
                set_pending();
            }
            // simply triggers a context switch
            sys::call_api::TrapReason::YieldTask => {
                set_pending();
            }
            // endpoint for every task. sets it's state
            // to completed so the scheduler skips this
            // task when searching for runnable task.
            sys::call_api::TrapReason::TerminateTask => {
                task_control::terminate_task();
                set_pending();
            }
            // disables task scheduling at all so the calling
            // task can finish its critical operation without getting disturbed
            sys::call_api::TrapReason::EnableRt => {
                disable_systick();
                return;
            }
            // enables task scheduling. by forgetting the calling task of
            // "enablert" would run forever or hang in terminated state
            sys::call_api::TrapReason::DisableRt => {
                enable_systick();
                return;
            }
            // writes a string to standard out, which is UART with baud 9600
            // at PA9 TX / PA10 RX (page 45 stm 32 mapping doc)
            sys::call_api::TrapReason::WriteStdOut => {
                let str_start = trap_meta_info.payload;
                print_from_ptr(str_start as *mut u8);
                enable_systick();
            }
            _ => {
                // not implemented yet...
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn DefaultExceptionHandler() {
    loop {}
}

///
/// Interrupt Service Routine when timer 3 cnt register reaches
/// value in timer 3 capture compare register 1.
///
#[no_mangle]
pub extern "C" fn Tim3Interrupt() {
    // tim3 isr has much lower priority than systick, so it is necessary to disable the systick
    // until sleeping task has successfully restored. otherwise it may occur that the restore
    // gets interrupted by systick and the overlaying context switch destroys the task switch workflow
    disable_systick();
    save_task_context();
    tim3::stop();

    // clear interrupt flag to prevent hanging in ISR
    tim3::clear_uif();

    // reset counter value to 0
    tim3::flush();

    load_sleeping_task();
    enable_systick();
}

///
/// Manually create a section which points to the adress of
/// the reset function.
///
#[link_section = ".vector_table.reset"]
#[no_mangle]
pub static RESET: [VectorDivergentFn; 1] = [VectorDivergentFn { handler: Reset }];

#[link_section = ".vector_table.exceptions"]
#[no_mangle]
pub static EXCEPTIONS: [Vector; 52] = [
    Vector { handler: NMI },
    Vector { handler: HardFault },
    Vector { handler: MemManage },
    Vector { handler: BusFault },
    Vector {
        handler: UsageFault,
    },
    Vector { reserved: 0 },
    Vector { reserved: 0 },
    Vector { reserved: 0 },
    Vector { reserved: 0 },
    Vector { handler: SVCall },
    Vector { reserved: 0 },
    Vector { reserved: 0 },
    Vector { handler: PendSV },
    Vector { handler: SysTick },
    Vector { reserved: 0 }, //wwdg pos 0
    // why reading docs when you can try'n error?? ;p
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
    Vector {
        handler: Tim3Interrupt,
    },
];
