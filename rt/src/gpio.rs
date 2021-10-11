pub mod gpio_driver {
    use core::ptr;

    // p52
    const GPIOA_BASE: u32 = 0x4800_0000;
    const GPIOB_BASE: u32 = 0x4800_0400;
    const GPIOC_BASE: u32 = 0x4800_0800;

    enum OutputTypes {
        PushPull,
        OpenDrain,
    }

    pub enum ModerTypes {
        InputMode,
        GeneralPurposeOutputMode,
        AlternateFunctionMode,
        AnalogMode
    }

    struct GpioConfig {
        gpio_port: u32,
        gpio_pin: u8,
        gpio_port_mode: u32,
        gpio_output_type: u32,
        gpio_output_data_register: u32,
    }


    pub fn get_port(port_mnemonic: &str) -> u32 {
        match port_mnemonic {
            "A" => GPIOA_BASE,
            "B" => GPIOB_BASE,
            "C" => GPIOC_BASE,
            _ => GPIOA_BASE
        }
    }

    pub fn set_moder(port_base: u32, moder_type: ModerTypes, pin: u8) {
        let gpiox_moder_offset = 0x00;
        let mut gpiox_moder = port_base | gpiox_moder_offset;

        // 32 bit register
        let mut current_register_content: u32;

        unsafe {
            current_register_content = ptr::read_volatile(gpiox_moder as *const u32);
        }
        // clear bits
        current_register_content &= !(0b11 as u32) << pin * 2;
        
        
        let updated_register_content = match moder_type {
            // clear bit
            ModerTypes::InputMode => {
                current_register_content |= (0b00 as u32) << pin * 2;
                current_register_content
            }
            // set bit
            ModerTypes::GeneralPurposeOutputMode => {
                current_register_content |= (0b01 as u32) << pin * 2;
                current_register_content
            }
            ModerTypes::AlternateFunctionMode => {
                current_register_content |= (0b10 as u32) << pin * 2;
                current_register_content
            }
            ModerTypes::AnalogMode => {
                current_register_content |= (0b11 as u32) << pin * 2;
                current_register_content
            } 
        };
        unsafe {
            ptr::write_volatile(gpiox_moder as *mut u32, updated_register_content);
        }

    }

    // p237 -> 11.4.2 GPIO port output type register
    fn set_otyper(port_base: u32, output_type: OutputTypes, pin: u8) {
        let gpiox_otyper_offset = 0x04;
        let mut gpiox_otyper = port_base | gpiox_otyper_offset;

        // 32 bit register
        let mut current_register_content: u32;

        unsafe {
            current_register_content = ptr::read_volatile(gpiox_otyper as *const u32);
        }

        let updated_register_content = match output_type {
            // clear bit
            OutputTypes::PushPull => {
                current_register_content &= !(1 as u32) << pin;
                current_register_content
            }
            // set bit
            OutputTypes::OpenDrain => {
                current_register_content |= (1 as u32) << pin;
                current_register_content
            }
        };
        unsafe {
            ptr::write_volatile(gpiox_otyper as *mut u32, updated_register_content);
        }
    }
}