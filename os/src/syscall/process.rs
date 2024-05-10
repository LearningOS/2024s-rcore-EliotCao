//! Process management syscalls
use crate::{
    config::{MAX_SYSCALL_NUM, PAGE_SIZE}, 
    mm::{translated_byte_buffer, MapPermission},
    task::{
        change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus, 
        current_user_token, get_running_time, get_syscall_times, get_task_status, mmap, munmap, 
    }, 
    timer::get_time_us
};
use core::mem::size_of;

/// Time with seconds and microseconds.
#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    /// The number of seconds.
    pub sec: usize,
    /// The number of microseconds.
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    pub status: TaskStatus,
    /// The numbers of syscall called by task
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    pub time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let buffers =
        translated_byte_buffer(current_user_token(), ts as *const u8, size_of::<TimeVal>());
    let us = get_time_us();
    // let time_val = TimeVal {
    //     sec: us / 1_000_000,
    //     usec: us % 1_000_000,
    // };
    // let mut time_val_ptr = &time_val as *const _ as *const u8;
    // for buffer in buffers {
    //     unsafe {
    //         time_val_ptr.copy_to(buffer.as_mut_ptr(), buffer.len());
    //         time_val_ptr = time_val_ptr.add(buffer.len());
    //     }
    // }
    let tv_ptr = buffers[0].as_ptr() as *mut TimeVal;
    unsafe {
        (*tv_ptr).sec = us / 1_000_000;
        (*tv_ptr).usec = us % 1_000_000;
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info NOT IMPLEMENTED YET!");
    let buffers = 
        translated_byte_buffer(current_user_token(), ti as *const u8, size_of::<TaskInfo>());
    let ti_ptr = buffers[0].as_ptr() as *mut TaskInfo;
    unsafe {
        // *ti_ptr = task_info;
        (*ti_ptr).status = get_task_status();
        (*ti_ptr).syscall_times = get_syscall_times();
        (*ti_ptr).time = get_running_time();
    }
    0
}

/// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    if start % PAGE_SIZE != 0 {
        return -1;
    }
    if port & 0x7 == 0 || port & !0x7 != 0 {
        return -1
    }
    let permission = MapPermission::from_bits((port as u8) << 1).unwrap() | MapPermission::U;
    mmap(start.into(), (start + len).into(), permission)
}

/// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    if start % PAGE_SIZE != 0 || len % PAGE_SIZE != 0 {
        return -1
    }
    munmap(start.into(), (start + len).into())
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
