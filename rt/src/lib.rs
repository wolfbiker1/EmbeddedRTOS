#![no_main]
#![no_std]
#![feature(asm)]
pub mod dev;

pub mod ctrl;
mod interrupts;
pub mod mem;
pub mod sched;

use core::panic::PanicInfo;
use core::ptr;
use interrupts::systick;

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
    let rcc_ahbenr = 0x40021000 | 0x14;
    unsafe { ptr::write_volatile(rcc_ahbenr as *mut u32, 1 << 17 | 1 << 21) }
    
    
    // TIM2EN -> p 166
    let rcc_apb1enr: u32 = 0x40021000 | 0x1C;
    unsafe {
        let existing_value = ptr::read_volatile(rcc_apb1enr as *mut u32);
        ptr::write_volatile(rcc_apb1enr as *mut u32, existing_value | 0b1);
    }
    
    // TIM2 RESET -> p 166
    let rcc_apb1rstr: u32 = 0x40021000 | 0x10;
    unsafe {
        let existing_value = ptr::read_volatile(rcc_apb1rstr as *mut u32);
        ptr::write_volatile(rcc_apb1rstr as *mut u32, existing_value | 0b1);
    }

    // USART1EN -> p 166
    let rcc_apb2enr: u32 = 0x4002_1000 | 0x18;
    unsafe {
        let existing_value = ptr::read_volatile(rcc_apb2enr as *mut u32);
        ptr::write_volatile(rcc_apb2enr as *mut u32, existing_value | (0b1 << 14 | 0b1));
    }
}

fn enable_serial_printing() {

    let gpio_port_a0 = dev::gpio_driver::GpioX::new("A", 9);
    gpio_port_a0.set_moder(dev::gpio_types::ModerTypes::AlternateFunctionMode);
    gpio_port_a0.set_otyper(dev::gpio_types::OutputTypes::PushPull);
    gpio_port_a0.into_af(7);


    let usart1 = dev::uart::new(1, 9600);
    usart1.enable();
}

pub fn print_k(msg: &str) {
    let usart2_tdr = 0x4001_3800 | 0x28;
    let usart2_isr = 0x4001_3800 | 0x1C;

    for c in msg.chars() {
        unsafe {
            ptr::write_volatile(usart2_tdr as *mut u32, c as u32);
            while !((ptr::read_volatile(usart2_isr as *mut u32) & 0x80) != 0) {}
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn Reset() -> ! {
    setup_clock_system();
    enable_gpio_e_leds();
    enable_serial_printing();
    systick::STK::set_up_systick(200);
    sched::scheduler::init_task_mng();
    print_k("hello from rtos!...\n\r");
    unsafe {
        asm!("bkpt");
    }
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
    fn __save_process_context();
    pub fn __exec(x: u32);
    pub fn __exec_kernel(x: u32);
    pub fn __set_exec_mode(y: u32);
    pub fn __get_msp_entry();
}

#[no_mangle]
pub extern "C" fn SysTick() {
    // TODO: reduce overhead and make code more clear!!
    // if !IS_WRITING.load(Ordering::Relaxed) {
        let tim2_cr1: u32 = 0x4000_0000 | 0x00;
    unsafe {
        let existing_value = ptr::read_volatile(tim2_cr1 as *mut u32);
        ptr::write_volatile(tim2_cr1  as *mut u32, existing_value | 0b1);
    }
    unsafe {
        __get_msp_entry();
        let msp_val: u32;
        asm! ("mov {}, r0", out(reg) msp_val);
        sched::scheduler::set_msp_entry(msp_val);
    }
    if sched::scheduler::usr_is_running() {
        unsafe {
            __save_process_context();
        }
        sched::task_control::update_tasks_ptr(ctrl::control::read_process_stack_ptr());
    }

    sched::scheduler::context_switch();
    if sched::scheduler::usr_is_running() {
        unsafe {
            __exec(sched::scheduler::get_msp_entry());
            ctrl::control::__load_process_context();
        }
    }
        // end measure 
        unsafe {
            let existing_value = ptr::read_volatile(tim2_cr1 as *mut u32);
            ptr::write_volatile(tim2_cr1  as *mut u32, existing_value & !(0b1));
            asm!("bkpt");
        }
        
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
