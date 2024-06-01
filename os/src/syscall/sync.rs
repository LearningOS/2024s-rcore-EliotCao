use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
use alloc::vec;
/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_status.push(1);
        for task in process_inner.tasks.iter() {
            if let Some(task) = task {
                let mut task_inner = task.inner_exclusive_access();
                if let Some(task_res) = task_inner.res.as_mut() {
                    task_res.mutex_require.push(0);
                    task_res.mutex_own.push(0);
                }
            }
        }
        process_inner.mutex_list.len() as isize - 1
    }
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    if process_inner.deadlock_detect {
        if let Some(current_task) = current_task() {
            let mut current_task_inner = current_task.inner_exclusive_access();
            if let Some(current_task_res) = current_task_inner.res.as_mut() {
                current_task_res.mutex_require[mutex_id] += 1; 
            }
        }
        let tasks_len = process_inner.tasks.len();
        let mut finish = vec![false;tasks_len];
        let mut work = process_inner.mutex_status.clone();

        for _ in 0..tasks_len  {
            let mut flag = false;
            // Find at least one task that can be completed in each round.
            for i in 0..tasks_len {
                if let Some(task) = &process_inner.tasks[i] {
                    if finish[i] {
                        continue;
                    }
                    let tmp_task_inner = task.inner_exclusive_access();
                    if let Some(tmp_task_res) = tmp_task_inner.res.as_ref() {
                        if  tmp_task_res.mutex_require.iter().zip(work.iter()).all(|(a,b)| a <= b) {
                            finish[i] = true;
                            for (work, have) in work.iter_mut().zip(tmp_task_res.mutex_own.iter()) {
                                *work += have;
                            } 
                            flag = true;
                            
                        } else {
                            continue;
                        }
                    }
                }
            }
            // If no task can complete, break.
            if flag == false {
                break;
            }
        }
        if finish.iter().any(|&x| x == false) {
            let current_task = current_task().unwrap();
            let mut current_task_inner = current_task.inner_exclusive_access();
            if let Some(current_task_res) = current_task_inner.res.as_mut() {
                current_task_res.mutex_require[mutex_id] -= 1;
            }
            return -0xDEAD;
        }
    }
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.lock();
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.mutex_status[mutex_id] -= 1;
    if let Some(current_task) = current_task() {
        let mut current_task_inner = current_task.inner_exclusive_access();
        if let Some(current_task_res) = current_task_inner.res.as_mut() {
            current_task_res.mutex_own[mutex_id] += 1;
            current_task_res.mutex_require[mutex_id] -= 1;
        }
    }
    0
}
/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    if let Some(current_task) = current_task() {
        let mut current_task_inner = current_task.inner_exclusive_access();
        if let Some(current_task_res) = current_task_inner.res.as_mut() {
            process_inner.mutex_status[mutex_id] += 1;
            current_task_res.mutex_own[mutex_id] -= 1;
        }
    }
    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_status.push(res_count as i32);
        for task in process_inner.tasks.iter() {
            if let Some(task) = task {
                let mut task_inner = task.inner_exclusive_access();
                if let Some(task_res) = task_inner.res.as_mut() {
                    task_res.semaphore_require.push(0);
                    task_res.semaphore_own.push(0);
                }
            }
        }
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    process_inner.semaphore_status[sem_id] += 1;
    if let Some(current_task) = current_task() {
        let mut current_task_inner = current_task.inner_exclusive_access();
        if let Some(current_task_res) = current_task_inner.res.as_mut() {
            current_task_res.semaphore_own[sem_id] -= 1; 
        }
    }
    drop(process_inner);
    sem.up();
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    if process_inner.deadlock_detect {
        if let Some(current_task) = current_task() {
            let mut current_task_inner = current_task.inner_exclusive_access();
            if let Some(current_task_res) = current_task_inner.res.as_mut() {
                current_task_res.semaphore_require[sem_id] += 1; 
            }
        }
        let mut finish = vec![false;process_inner.tasks.len()];
        let mut work = process_inner.semaphore_status.clone();
        let tasks_len = process_inner.tasks.len();
        for _ in 0..tasks_len  {
            let mut flag = false;
            // Find at least one task that can be completed in each round.
            for i in 0..tasks_len {
                if let Some(task) = &process_inner.tasks[i] {
                    if finish[i] {
                        continue;
                    }
                    let tmp_task_inner = task.inner_exclusive_access();
                    if let Some(tmp_task_res) = tmp_task_inner.res.as_ref() {
                        if  tmp_task_res.semaphore_require.iter().zip(work.iter()).all(|(a,b)| a <= b) {
                            finish[i] = true;
                            for (work, have) in work.iter_mut().zip(tmp_task_res.semaphore_own.iter()) {
                                *work += have;
                            }
                            flag = true;
                        }
                    } else {
                        finish[i] = true;
                        continue;
                    }
                }
                
            }
            if flag == false {
                break;
            } 
        }
        if finish.iter().any(|&x| x == false) {
            if let Some(current_task) = current_task() {
                let mut current_task_inner = current_task.inner_exclusive_access();
                if let Some(current_task_res) = current_task_inner.res.as_mut() {
                    current_task_res.semaphore_require[sem_id] -= 1;
                }
            }
            return -0xDEAD;
        }
    }
    
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    sem.down();
    if let Some(current_task) = current_task() {
        let mut current_task_inner = current_task.inner_exclusive_access();
        if let Some(current_task_res) = current_task_inner.res.as_mut() {
            let mut process_inner = process.inner_exclusive_access();
            current_task_res.semaphore_own[sem_id] += 1;
            current_task_res.semaphore_require[sem_id] -= 1;
            process_inner.semaphore_status[sem_id] -= 1;
        }
    }
    0
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
///
/// YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect NOT IMPLEMENTED");
    if enabled == 1 {
        let process = current_process();
        let mut process_inner = process.inner_exclusive_access();
        process_inner.deadlock_detect = true;
        0
    } else if enabled == 0 {
        let process = current_process();
        let mut process_inner = process.inner_exclusive_access();
        process_inner.deadlock_detect = false;
        0
    } else {
        -1
    }
}
