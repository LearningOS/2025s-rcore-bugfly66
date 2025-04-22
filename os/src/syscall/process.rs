//! Process management syscalls


use crate::config::PAGE_SIZE;
use crate::mm::PTEFlags;
use crate::mm::PageTable;
use crate::mm::VirtPageNum;
use crate::task::current_task;
use crate::{
    mm::frame_alloc,
    task::{change_program_brk, exit_current_and_run_next, suspend_current_and_run_next},
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
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
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    -1
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(_trace_request: usize, _id: usize, _data: usize) -> isize {
    trace!("kernel: sys_trace");
    -1
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    let pa = frame_alloc();
    // let port = (_port<<1)&7;
    // let flags = PTEFlags::from_bits(port.try_into().unwrap()).unwrap();
    let mut flags = PTEFlags::empty();
    flags.set(PTEFlags::R, if _port&1 ==1 {true} else {false});
    flags.set(PTEFlags::W, if _port&2 ==1 {true} else {false});
    flags.set(PTEFlags::X, if _port&4 ==1 {true} else {false});
    flags.set(PTEFlags::V,true);
    flags.set(PTEFlags::U,true);
    match pa {
        Some(ft) => {
            // let mut page_table = PageTable::from_token( current_user_token());
            // let aa = page_table.translate(_start.into()).unwrap();
            // print!("{:?}",aa.ppn());
            // page_table.map(VirtPageNum::from(_start), ft.ppn, flags);
            let  current_task = current_task();
            current_task.memory_set.map_va_pa(VirtPageNum::from(_start), ft.ppn, flags);
            return 0;
        }
        None => {
            return -1;
        }
    }

    // return -1;
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    let mut page_table = PageTable::new();
    for i in 0..(_len % PAGE_SIZE) {
        match page_table.translate((_start+i).into()) {
            Some(_) => {
                page_table.unmap((_start+i).into());
            }
            None => return -1,
        }
    }

    return 0;
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
