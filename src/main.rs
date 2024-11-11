//! This example uses the RP Pico W board Wifi chip (cyw43).
//! Connects to specified Wifi network and creates a TCP endpoint on port 1234.

#![no_std]
#![no_main]
#![allow(async_fn_in_trait)]
#![feature(type_alias_impl_trait)]

use heapless::String;

use cyw43::JoinOptions;
use cyw43_pio::PioSpi;
use defmt::*;
use embassy_executor::Spawner;
use embassy_net::{Config, StackResources};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_rp::bind_interrupts;
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_time::{Duration, Timer};
use picoserve::routing::{post, get_service};
use rand::RngCore;
use static_cell::{make_static, StaticCell};

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

type AppRouter = impl picoserve::routing::PathRouter;

const WEB_TASK_POOL_SIZE: usize = 8;

#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
async fn web_task(
    id: usize,
    stack: embassy_net::Stack<'static>,
    app: &'static picoserve::Router<AppRouter>,
    config: &'static picoserve::Config<Duration>,
) -> ! {
    info!("Starting webtask {}", id);

    let port = 80;
    let mut tcp_rx_buffer = [0; 1024];
    let mut tcp_tx_buffer = [0; 1024];
    let mut http_buffer = [0; 2048];

    picoserve::listen_and_serve(
        id,
        app,
        config,
        stack,
        port,
        &mut tcp_rx_buffer,
        &mut tcp_tx_buffer,
        &mut http_buffer,
    )
    .await
}

static SIGNAL : Signal<CriticalSectionRawMutex, ()> = Signal::new();

static mut BUTN_COUNT : i32 = 0;

async fn butn() -> &'static str {
    SIGNAL.signal(());
    "{\"status\": \"ok\"}"
}

#[embassy_executor::task]
async fn vrata_task(mut pin : Output<'static>) -> ! {
    loop {
        let _ = SIGNAL.wait().await;
        pin.set_high();
        Timer::after_millis(100).await;
        pin.set_low();
        Timer::after_secs(2).await;
    }
}

#[embassy_executor::task]
async fn led_task(mut pin : Output<'static>) -> ! {
    loop {
        pin.set_high();
        Timer::after_millis(100).await;
        pin.set_low();
        Timer::after_secs(10).await;
    }
}


#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello World!");

    let p = embassy_rp::init(Default::default());
    let mut rng = RoscRng;

    let fw = include_bytes!("../firmware/43439A0.bin");
    let clm = include_bytes!("../firmware/43439A0_clm.bin");

    let ssid = include_str!("../wifi/ssid").trim_end();
    let psk = include_str!("../wifi/psk").trim_end();

    // To make flashing faster for development, you may want to flash the firmwares independently
    // at hardcoded addresses, instead of baking them into the program with `include_bytes!`:
    //     probe-rs download 43439A0.bin --binary-format bin --chip RP2040 --base-address 0x10100000
    //     probe-rs download 43439A0_clm.bin --binary-format bin --chip RP2040 --base-address 0x10140000
    //let fw = unsafe { core::slice::from_raw_parts(0x10100000 as *const u8, 230321) };
    //let clm = unsafe { core::slice::from_raw_parts(0x10140000 as *const u8, 4752) };

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        p.DMA_CH0,
    );

    let mut led = Output::new(p.PIN_10, Level::High);

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    unwrap!(spawner.spawn(cyw43_task(runner)));

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let mut dhcpv4 = embassy_net::DhcpConfig::default();
    const MAX_HOSTNAME_LEN: usize = 32;
    let hostname: String<MAX_HOSTNAME_LEN> = String::try_from("horman").expect("hostname");
    dhcpv4.hostname = Some(hostname);
    let config = Config::dhcpv4(dhcpv4);
    //let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
    //    address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 69, 2), 24),
    //    dns_servers: Vec::new(),
    //    gateway: Some(Ipv4Address::new(192, 168, 69, 1)),
    //});

    // Generate random seed
    let seed = rng.next_u64();

    // Init network stack
    static RESOURCES: StaticCell<StackResources<20>> = StaticCell::new();
    let (stack, runner) = embassy_net::new(
        net_device,
        config,
        RESOURCES.init(StackResources::new()),
        seed,
    );

    unwrap!(spawner.spawn(net_task(runner)));

    let vrata_pin = Output::new(p.PIN_11, Level::Low);
    unwrap!(spawner.spawn(vrata_task(vrata_pin)));

    

    loop {
        match control.join(ssid, JoinOptions::new(psk.as_bytes())).await {
            Ok(_) => break,
            Err(err) => {
                info!("join failed with status={}", err.status);
            }
        }
    }

    led.set_low();

    // Wait for DHCP, not necessary when using static IP
    info!("waiting for DHCP...");
    while !stack.is_config_up() {
        Timer::after_millis(100).await;
    }
    info!("DHCP is now up!");

    fn make_app() -> picoserve::Router<AppRouter> {
        picoserve::Router::new()
            .route(
                "/vrata/",
                get_service(picoserve::response::File::html(include_str!(
                    "../site/index.html"
                ))),
            )
            .route(
                "/vrata/index.css",
                get_service(picoserve::response::File::css(include_str!(
                    "../site/index.css"
                ))),
            )
            .route(
                "/vrata/index.js",
                get_service(picoserve::response::File::javascript(include_str!(
                    "../site/index.js"
                ))),
            )
            .route(
            "/vrata/api/butn",
            post(butn))
    }

    let app = make_static!(make_app());

    let config = make_static!(picoserve::Config::new(picoserve::Timeouts {
        start_read_request: Some(Duration::from_secs(5)),
        read_request: Some(Duration::from_secs(1)),
        write: Some(Duration::from_secs(1)),
    })
    .keep_connection_alive());

    for id in 0..WEB_TASK_POOL_SIZE {
        spawner.must_spawn(web_task(id, stack, app, config));
    }
    unwrap!(spawner.spawn(led_task(led)));

}
