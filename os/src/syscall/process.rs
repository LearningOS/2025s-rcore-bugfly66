//! Process management syscalls
use crate::config::PAGE_SIZE;
use crate::mm::frame_alloc;
use crate::mm::VirtAddr;
use crate::mm::VirtPageNum;
use crate::timer::get_time_us;
use crate::mm::PTEFlags;
use crate::{
    loader::get_app_data_by_name,
    mm::{translated_refmut, translated_str},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next,
    },
};
use alloc::sync::Arc;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel:pid[{}] sys_yield", current_task().unwrap().pid.0);
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let task = current_task().unwrap();
        task.exec(data);
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    trace!(
        "kernel::pid[{}] sys_waitpid [{}]",
        current_task().unwrap().pid.0,
        pid
    );
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_get_time NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let mem_set = current_task().unwrap().inner_exclusive_access().memory_set;
    let pte = mem_set
        .translate(VirtPageNum::from(VirtAddr::from(
            (ts as usize) & (!0 << 12 as usize) as usize,
        )));
    match pte {
        Some(x) => {
            let mut pa = x.ppn().0 as *mut TimeVal;
            pa = ((pa as usize) << 12).wrapping_add((ts as usize) & 0xfff) as *mut TimeVal;
            unsafe {
                *pa = TimeVal {
                    sec: us / 1_000_000,
                    usec: us % 1_000_000,
                };
            }
        }
        None => {
            // sys_mmap(ts as usize, 4096, 3);
            // let _pte2 = current_memory_set()
            //     .exclusive_access()
            //     .translate(VirtPageNum::from(ts as usize));
            panic!("TimeVal's address is not readable or writable");
        }
    }
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    // trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    if _port & !0x7 != 0 {
        return -1;
    }
    if _port & 0x7 == 0 {
        return -1;
    }
    // println!("_start&0xfff{}",_start&0xfff);
    // start 按页大小对齐
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

                let vpn = VirtPageNum::from(VirtAddr::from((_start + i * PAGE_SIZE)&(!(0<<12 as usize)) as usize));
                // println!("#######vpn1:{:#x?},2:{:#x?}",vpn2.0,(_start + i * PAGE_SIZE)&(!((0<<12)as usize)));
                let mem_set = current_task().unwrap().inner_exclusive_access().memory_set;
                let ppn = mem_set.translate(vpn);
                if let Some(x) = ppn {
                    if x.ppn().0 != 0 {
                        // println!("already exist:{:#x?},ppn:{:#x?}", vpn.0, x.ppn().0);
                        return -1;
                    }
                }

                set_current_page_table(vpn, ft.ppn, flags);
                // let ppn = current_memory_set().exclusive_access().translate(vpn);
                // println!("not exist:{:#x?},ppn:{:#x?}", vpn.0, ppn.unwrap().ppn().0);
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
    // trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    //
    if _start & 0xfff != 0 {
        return -1;
    }
    for i in 0..((_len + PAGE_SIZE - 1) / PAGE_SIZE) {
        let vpn = VirtPageNum::from(VirtAddr::from((_start + i * PAGE_SIZE)&(!(0<<12 as usize)) as usize));
        let mem_set = current_task().unwrap().inner_exclusive_access().memory_set;
        let pte = mem_set.translate(vpn)
            .unwrap();
        if pte.is_valid() && pte.ppn().0 != 0 {
            mem_set(vpn);
        } else {
            return -1;
        }
    }

    return 0;
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(_path: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_spawn NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let res = sys_fork();
    if res == -1 {
        return -1;
    } else {
        sys_exec(_path);
    }
    res
}

// YOUR JOB: Set task priority.
pub fn sys_set_priority(_prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    -1
}
