use anyhow::{Context, Result};
use esp_idf_svc::hal::cpu::Core;
use esp_idf_svc::hal::task::thread::ThreadSpawnConfiguration;
use esp_idf_svc::sys::{uxTaskGetStackHighWaterMark2, vTaskList};
use std::thread::JoinHandle;

pub fn spawn<F>(name: &'static str, pin_to_core: Core, thread_body: F) -> Result<JoinHandle<()>>
where
    F: FnOnce() -> Result<()> + Send + 'static,
{
    let stack_size = 16384;

    let thread_configuration = ThreadSpawnConfiguration {
        name: Some(name.as_bytes()),
        stack_size,
        priority: 15,
        pin_to_core: Some(pin_to_core),
        ..Default::default()
    };

    thread_configuration
        .set()
        .context("failed to configure thread")?;

    Ok(std::thread::Builder::new()
        .stack_size(stack_size)
        .spawn(move || {
            log_remaining_stack(name);

            unsafe {
                let mut buffer = [0u8; 512];
                vTaskList(buffer.as_mut_ptr() as *mut i8);
                log::info!("Task list:\n{}", String::from_utf8_lossy(&buffer));
            }

            if let Err(e) = thread_body() {
                log::error!("[{}] thread panic: {:?}", name, e);
            }
        })
        .context("failed to spawn thread")?)
}

pub fn log_remaining_stack(task_name: &str) {
    let remaining = unsafe { uxTaskGetStackHighWaterMark2(std::ptr::null_mut()) };
    log::info!("Remaining stack for {}: {} words", task_name, remaining);
}
