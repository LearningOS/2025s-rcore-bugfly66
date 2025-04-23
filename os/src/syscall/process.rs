//! Process management syscalls

use crate::mm::VirtAddr;

use crate::config::PAGE_SIZE;
use crate::mm::PTEFlags;
use crate::mm::VirtPageNum;
use crate::task::current_memory_set;
// use crate::mm::VirtPageNum;
use crate::task::set_current_page_table;
use crate::task::unmap_current_page_table;
// use crate::task::current_memory_set;
// use crate::task::current_task;
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
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(_trace_request: usize, _id: usize, _data: usize) -> isize {
    trace!("kernel: sys_trace");
    let id_addr = id as *mut u8;
    if trace_request == 0 {
        let res: isize = unsafe { core::ptr::read_volatile(id as *const u8) as isize };
        return res;
    } else if trace_request == 1 {
        unsafe {
            core::ptr::write_volatile(id_addr, data as u8);
        };
        return 0;
    } else {
        return get_current_task_syscall_times()[id] as isize;
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    if _port & !0x7 != 0 {
        return -1;
    }
    if _port & 0x7 == 0 {
        return -1;
    }
    // println!("_start&0xfff{}",_start&0xfff);
    if _start & 0xfff != 0 {
        return -1;
    }

    let mut flags = PTEFlags::empty();
    flags.set(PTEFlags::R, if _port & 1 == 1 { true } else { false });
    flags.set(PTEFlags::W, if _port >> 1 & 1 == 1 { true } else { false });
    flags.set(PTEFlags::X, if _port >> 2 & 1 == 1 { true } else { false });
    flags.set(PTEFlags::V, true);
    flags.set(PTEFlags::U, true);
    flags.set(PTEFlags::A, true);
    flags.set(PTEFlags::D, true);
    flags.set(PTEFlags::G, true);
    // let res = 0;
    for i in 0..((_len + PAGE_SIZE - 1) / PAGE_SIZE) {
        let pa = frame_alloc();
        match pa {
            Some(ft) => {
                println!("{:#x?}", _start + i * PAGE_SIZE);

                let vpn = VirtPageNum::from(VirtAddr::from(_start + i * PAGE_SIZE));
                let ppn = current_memory_set().exclusive_access().translate(vpn);
                if let Some(x) = ppn {
                    if x.ppn().0 != 0 {
                        println!("already exist:{:#x?},ppn:{:#x?}", vpn.0, x.ppn().0);
                        return -1;
                    }
                }

                set_current_page_table(vpn, ft.ppn, flags);
                let ppn = current_memory_set().exclusive_access().translate(vpn);
                println!("not exist:{:#x?},ppn:{:#x?}", vpn.0, ppn.unwrap().ppn().0);
            }
            None => {
                return -1;
            }
        }
    }

    return 0;
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    // let mut page_table = PageTable::new();
    if _start & 0xfff != 0 {
        return -1;
    }
    for i in 0..((_len + PAGE_SIZE - 1) / PAGE_SIZE) {
        // let mem = current_memory_set();
        let vpn = VirtPageNum::from(VirtAddr::from(_start + i * PAGE_SIZE));
        let pte = current_memory_set()
            .exclusive_access()
            .translate(vpn)
            .unwrap();
        if pte.is_valid() && pte.ppn().0 != 0 {
            unmap_current_page_table(vpn);
        } else {
            return -1;
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
