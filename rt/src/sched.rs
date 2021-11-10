pub mod task_control {
    use core::sync::atomic::{AtomicU32, Ordering};
    pub enum TaskStates {
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

    const TCB_START: u32 = 0x2000_0200;
    static HEAP_SIZE: AtomicU32 = AtomicU32::new(0);
    pub static CURRENT_TASK: AtomicU32 = AtomicU32::new(0);
    const TCB_SIZE: u32 = core::mem::size_of::<TCB>() as u32;


    pub fn set_task_state(new_state: TaskStates) {
        match get_current_tcb() {
            Some(t) => t.state = new_state,
            None => {}
        }
    }

    pub fn next_process() -> u32 {
        // hint: fetch_add to load!
        let current = CURRENT_TASK.fetch_add(1, Ordering::Relaxed) as u32;
        let next = (current + 1) % HEAP_SIZE.load(Ordering::Relaxed);
        let target_tcb_adress = (next * TCB_SIZE) + TCB_START;
        let tcb = unsafe { &mut *(target_tcb_adress as *mut Option<TCB>) };

        // CURRENT_TASK.store(next, Ordering::Relaxed);
        let sp_of_next_process: u32;
        match tcb {
            Some(t) => match t.state {
                TaskStates::READY => {
                    sp_of_next_process = t.sp;
                    t.state = TaskStates::RUNNING
                }
                TaskStates::BLOCKED | TaskStates::TERMINATED  => {
                    sp_of_next_process = next_process();
                }
                _ => {
                    sp_of_next_process = t.sp;
                }
            },
            None => {
                sp_of_next_process = 0x00;
            }
        }
        sp_of_next_process
    }

    fn get_current_tcb<'a>() -> &'a mut Option<TCB> {
        let current = CURRENT_TASK.load(Ordering::Relaxed) as u32;
        let target_tcb_adress = (current * TCB_SIZE) + TCB_START;
        unsafe { &mut *(target_tcb_adress as *mut Option<TCB>) }
    }

    pub fn update_sp(new_sp: u32) {
        match get_current_tcb() {
            Some(t) => t.sp = new_sp,
            None => {}
        }
    }

    pub fn terminate_task() {
        match get_current_tcb() {
            Some(t) => t.state = TaskStates::TERMINATED,
            None => {}
        }
    }

    pub fn insert(stack_pointer: u32) -> u32 {
        let pid = HEAP_SIZE.load(Ordering::Relaxed);
        let entry_target = (pid * TCB_SIZE) + TCB_START;
        let tcb = unsafe { &mut *(entry_target as *mut Option<TCB>) };

        *tcb = Some(TCB {
            sp: stack_pointer,
            state: TaskStates::READY,
            pid,
        });

        HEAP_SIZE.fetch_add(1, Ordering::Relaxed);
        pid
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
    use super::task_control::{next_process, set_task_state, update_sp};

    pub fn immediate_start(addr: *const u32) {
        unsafe {
            __load_process_context(addr as u32);
        }
    }

    pub fn context_switch() {
        unsafe {
            // loads process stack pointer value into r0,
            // based on this adress registers r4 - r11 gets
            // pushed onto the stack. after finishing this operation,
            // the new value of r0 (it points now to lower adresses 
            // because registers get pushed onto it) gets assigned 
            // to psp.
            __save_process_context();

            // the newly written process stack pointer gets written
            // into the task control block of the process table 
            // for further restoring when needed
            update_sp(__get_current_psp());

            // the saved task's state gets changed from running 
            // to ready, because no other event blocks or terminates
            // the task
            set_task_state(task_control::TaskStates::READY);

            // the function next_process returns an u32 adress 
            // to the runnable successors task's stackpointer, which is
            // saved in the process table. the parameter gets saved
            // into r0, based from this value the registers r4 - r11
            // gets popped of the stack and written into the cpu's registers. 
            __load_process_context(next_process());
        }
    }
}
