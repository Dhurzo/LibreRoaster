use crate::application::service_container::ServiceContainer;
use crate::output::artisan::ArtisanFormatter;
use embassy_executor::task;
use embassy_time::{Duration, Timer};
use log::info;

/// Control loop task that uses the service container
/// This replaces the unsafe static mut access in the original main.rs
#[task]
pub async fn control_loop_task(_formatter: ArtisanFormatter) {
    info!("Roaster control loop started");

    loop {
        let current_time = embassy_time::Instant::now();

        // Execute control logic using service container
        let result = ServiceContainer::with_roaster(|roaster: &mut crate::control::RoasterControl| {
            // For testing: simulate temperatures
            let bean_temp = 25.0; // Room temp
            let env_temp = 22.0; // Room temp

            // Update temperatures
            if let Err(e) = roaster.update_temperatures(bean_temp, env_temp, current_time) {
                info!("Temperature update error: {:?}", e);
            }

            // Update control logic
            if let Err(e) = roaster.update_control(current_time) {
                info!("Control update error: {:?}", e);
            }

            // Store the result for async processing
            true
        });

        if let Err(e) = result {
            info!("Service container error in control loop: {:?}", e);
        }

        // Process output asynchronously outside the critical section
        if let Ok(_) = ServiceContainer::with_roaster(|_roaster: &mut crate::control::RoasterControl| {
            // This is a workaround - in production we'd need a proper async design
            // For now, we'll simulate the async call
        }) {
            // Would process output async here
        }

        Timer::after(Duration::from_millis(100)).await;
    }
}

/// Artisan UART handler task that uses the service container
/// This replaces the unsafe static mut access from original main.rs
#[task]
pub async fn artisan_uart_handler_task(_formatter: ArtisanFormatter) {
    info!("Artisan+ UART Handler Started");

    loop {
        Timer::after(Duration::from_millis(50)).await;

        // Process artisan commands using service container
        // For simplicity, we'll handle this more synchronously
        let result = ServiceContainer::with_artisan_input(|_artisan_input: &mut crate::input::ArtisanInput| {
            // This would need proper async integration
            // For now, we'll simulate the processing
            true // Indicate processing happened
        });

        if let Err(e) = result {
            info!("Service container error in UART handler: {:?}", e);
        }
    }
}