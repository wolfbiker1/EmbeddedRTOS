#![no_main]
#![no_std]
#![feature(asm)]
pub mod gpio;

pub mod ctrl;
mod interrupts;
pub mod mem;
pub mod sched;

use core::panic::PanicInfo;
use core::ptr;
use interrupts::systick;
use sched::scheduler::subroutine;

fn foo() {
    // turn on gpio clock
    // see p 166 -> IOPAEN
    let rcc_ahbenr = 0x40021000 | 0x14;
    unsafe { ptr::write_volatile(rcc_ahbenr as *mut u32, 1 << 17 | 1 << 21) }

    // see p 54 reg boundaries
    let gpio_port_e11 = gpio::gpio_driver::GpioX::new("E", 11);
    gpio_port_e11.set_moder(gpio::gpio_types::ModerTypes::GeneralPurposeOutputMode);
    gpio_port_e11.set_otyper(gpio::gpio_types::OutputTypes::PushPull);
    let gpio_port_e14 = gpio::gpio_driver::GpioX::new("E", 12);
    gpio_port_e14.set_moder(gpio::gpio_types::ModerTypes::GeneralPurposeOutputMode);
    gpio_port_e14.set_otyper(gpio::gpio_types::OutputTypes::PushPull);

    let gpio_port_e14 = gpio::gpio_driver::GpioX::new("E", 14);
    gpio_port_e14.set_moder(gpio::gpio_types::ModerTypes::GeneralPurposeOutputMode);
    gpio_port_e14.set_otyper(gpio::gpio_types::OutputTypes::PushPull);

    let gpio_port_e14 = gpio::gpio_driver::GpioX::new("E", 13);
    gpio_port_e14.set_moder(gpio::gpio_types::ModerTypes::GeneralPurposeOutputMode);
    gpio_port_e14.set_otyper(gpio::gpio_types::OutputTypes::PushPull);
}
#[no_mangle]
pub unsafe extern "C" fn Reset() -> ! {
    foo();
    systick::STK::set_up_systick(1000);
    sched::scheduler::set_up();
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

    extern "Rust" {
        fn main() -> !;
    }

    main()
}

#[link_section = ".vector_table.reset_vector"]
#[no_mangle]
pub static RESET_VECTOR: unsafe extern "C" fn() -> ! = Reset;

#[panic_handler]
fn panic(_panic: &PanicInfo<'_>) -> ! {
    loop {}
}

pub union Vector {
    reserved: u32,
    handler: unsafe extern "C" fn(),
}

extern "C" {
    fn NMI();
    fn HardFault();
    fn MemManage();
    fn BusFault();
    fn UsageFault();
    fn PendSV();
    pub fn __exec();
    pub fn __get_msp_entry();
}

#[no_mangle]
pub extern "C" fn SysTick() {
    // unsafe {
    // __get_msp_entry();
    // asm! ("mov {}, r0", out(reg) msp_val);
    // }
    // sched::scheduler::context_switch();
    unsafe {
        __get_msp_entry();
        let msp_val: u32;
        asm! ("mov {}, r0", out(reg) msp_val);
        sched::scheduler::set_msp_entry(msp_val);
    }
        // let qux = sched::scheduler::get_msp_entry();
    unsafe {
        asm!("bkpt");
        asm!(
            "
        bkpt
        push {{R4-R11}}    
        bkpt"
        );
    }
    

    // let f = 123;
    // let f1 = 103;
    // let f2 = 183;
    sched::scheduler::subroutine(0);
    unsafe {
        asm!(
            "
        mov R4, #0x666
        pop {{R4-R11}}
        bkpt"
        );
    }
    // unsafe {
    //     __exec();
    // }
}

#[no_mangle]
pub extern "C" fn SVCall() {
    sched::scheduler::run(0);
}

#[no_mangle]
pub extern "C" fn DefaultExceptionHandler() {
    loop {}
}

#[link_section = ".vector_table.exceptions"]
#[no_mangle]
pub static EXCEPTIONS: [Vector; 14] = [
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
];
