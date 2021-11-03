pub mod task_control {
    use crate::dev::uart::{self, print_str};
    use crate::{dev::uart::print_dec, mem};
    use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    enum TaskStates {
        READY,
        RUNNING,
        BLOCKED,
        TERMINATED,
    }

    #[repr(C)]
    #[repr(align(4))]
    pub struct TCB {
        sp: u32,
        state: TaskStates,
        pid: u32,
    }

    enum VecMeta {
        MAGIC,
        PREV,
        CURRENT,
        SIZE,
        FLUSH,
    }

    const TCB_START: u32 = 0x2000_0200;
    static HEAP_SIZE: AtomicU32 = AtomicU32::new(0);
    static CURRENT_TASK: AtomicU32 = AtomicU32::new(0);
    const TCB_SIZE: u32 = core::mem::size_of::<TCB>() as u32;

    const NUM_TASKS: u32 = 5;

    pub fn print() {
        let tcb_size = core::mem::size_of::<TCB>();
        let tcb_location = unsafe { core::ptr::read_volatile(TCB_START as *const u32) };
        for tcb_addr in
            (TCB_START..(TCB_START + (NUM_TASKS * tcb_size as u32) as u32)).step_by(tcb_size)
        {
            let tcb = unsafe { &mut *(tcb_addr as *mut Option<TCB>) };
            match tcb {
                Some(tcb) => {
                    uart::print_dec(tcb.pid);
                    uart::print_str("\n\r");
                    unsafe {
                        asm! {"bkpt"}
                    }
                }
                None => {}
            }
        }
    }

    pub fn next_process() -> u32 {
        let current = CURRENT_TASK.fetch_add(1, Ordering::Relaxed) as u32;
        let next = (current + 1) % HEAP_SIZE.load(Ordering::Relaxed);
        let target_tcb_adress = (next * TCB_SIZE) + TCB_START;
        let tcb = unsafe { &mut *(target_tcb_adress as *mut Option<TCB>) };

        CURRENT_TASK.store(next, Ordering::Relaxed);

        match tcb {
            Some(t) => t.sp,
            None => 0x00,
        }
    }

    pub fn current_process() -> u32 {
        let current = CURRENT_TASK.load(Ordering::Relaxed) as u32;
        let entry_target = (current * TCB_SIZE) + TCB_START;
        let tcb = unsafe { &mut *(entry_target as *mut Option<TCB>) };
        match tcb {
            Some(t) => t.sp,
            None => 0x00,
        }
    }

    pub fn update_sp(new_sp: u32) {
        let current = CURRENT_TASK.load(Ordering::Relaxed) as u32;
        let entry_target = (current * TCB_SIZE) + TCB_START;
        let tcb = unsafe { &mut *(entry_target as *mut Option<TCB>) };
        *tcb = Some(TCB {
            sp: new_sp,
            state: TaskStates::READY,
            pid: current,
        });
    }

    pub fn insert(stack_pointer: u32, pid: u32) {
        let entry_target = (HEAP_SIZE.load(Ordering::Relaxed) as u32 * TCB_SIZE) + TCB_START;
        let tcb = unsafe { &mut *(entry_target as *mut Option<TCB>) };

        *tcb = Some(TCB {
            sp: stack_pointer,
            state: TaskStates::READY,
            pid,
        });

        HEAP_SIZE.fetch_add(1, Ordering::Relaxed);
    }
}

pub mod scheduler {

    extern "C" {
        pub fn __write_psp(addr: u32);
        fn __save_process_context();
        fn __load_process_context(addr: u32);
        fn __get_current_psp() -> u32;
    }
    use crate::sched::task_control;

    use super::task_control::next_process;

    pub fn immediate_start(addr: *const u32) {
        unsafe {
            __load_process_context(addr as u32);
        }
    }

    pub fn context_switch() {
        unsafe {
            __save_process_context();
            task_control::update_sp(__get_current_psp());
            __load_process_context(next_process());
        }
    }
}
