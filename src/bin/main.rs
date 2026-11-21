#![no_std]
#![no_main]


#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use esp_hal::clock::CpuClock;
use esp_hal::timer::timg::TimerGroup;


use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};

use defmt::info;

use panic_rtt_target as _;



mod LED_Driver{
    use esp_hal::riscv::register::Permission::R;
use esp_hal::time::Rate;
    use esp_hal::gpio::Level;
    use esp_hal::rmt::{PulseCode, Rmt, TxChannelConfig, TxChannelCreator, Channel};
    const MAX_LED_COUNT: usize = 255;
    const LED_0: PulseCode = PulseCode::new(Level::High, 4, Level::Low, 9);
    const LED_1: PulseCode = PulseCode::new(Level::High, 8, Level::Low, 5);
    const LED_RET: PulseCode = PulseCode::end_marker();
    #[derive(Clone, Copy)]
    pub struct RGB {
        r: u8,
        g: u8,
        b: u8,
    }

    // Define some colors for the LEDs
    pub const LED_RED: RGB = RGB { r: 255, g: 0, b: 0 };
    pub const LED_GREEN: RGB = RGB { r: 0, g: 255, b: 0 };
    pub const LED_BLUE: RGB = RGB { r: 0, g: 0, b: 255 };
    pub const LED_WHITE: RGB = RGB { r: 255, g: 255, b: 255 };
    pub const LED_OFF: RGB = RGB { r: 0, g: 0, b: 0 };
    pub const LED_YELLOW: RGB = RGB { r: 255, g: 255, b: 0 };

    enum LED_Driver_State{
        Created,
        Initialized,
        Running,
        Stopped,
        Error,
    }
    enum LED_Function{
        Static,
        Dimmed(u8),
        Breathing(u8),
        Off,
    }

    struct LED_Driver{
        state: LED_Driver_State,
        function: LED_Function,
        rmt: Rmt<'_, esp_hal::Blocking>,
        tx_config: TxChannelConfig,
        channel: Channel<'_, esp_hal::Blocking, esp_hal::rmt::Tx>,
        colors: [RGB; MAX_LED_COUNT],
        led_count: usize,
    }

    impl LED_Driver{
        pub fn new() -> Self{
            LED_Driver{
                state: LED_Driver_State::Created,
                function: LED_Function::Off,
                channel: TxChannelConfig::default(),
                colors: [RGB{r:0,g:0,b:0}; MAX_LED_COUNT],
                led_count: 0,
            }
        }
        pub fn init(&mut self, rmt: Rmt<'_, esp_hal::Blocking>, led_pin: Peripherals){
            match self.state{
                LED_Driver_State::Created => {
                    self.channel = rmt.create_tx_channel(led_pin).unwrap();
                    self.state = LED_Driver_State::Initialized;
                },
                _ => {
                    info!("LED Driver is not in Created state, cannot initialize");
                    return;
                }
            }
        }

        pub fn set_color(&mut self, color: RGB){
            self.color = color;
        }

        pub fn set_function(&mut self, function: LED_Function){
            self.function = function;
        }

        pub fn set_function_Dimmed(&mut self, dim_level: u8){
            self.function = LED_Function::Dimmed(dim_level);
        }

        pub fn set_function_Breathing(&mut self, breath_freq: u8){
            self.function = LED_Function::Breathing(breath_freq);
        }

        pub fn set_function_Off(&mut self){
            self.function = LED_Function::Off;
        }

        pub fn set_function_Static(&mut self){
            self.function = LED_Function::Static;
        }

        pub fn start(&mut self){
            if self.state != LED_Driver_State::Initialized || self.state != LED_Driver_State::Stopped{
                info!("LED Driver is not in Initialized or Stopped state, cannot start");
                return;
            }
            self.state = LED_Driver_State::Running;
        }

        pub fn stop(&mut self){
            if self.state != LED_Driver_State::Running{
                info!("LED Driver is not in Running state, cannot stop");
                return;
            }
            self.state = LED_Driver_State::Stopped;
        }

        pub fn run(&mut self){
            if self.state != LED_Driver_State::Running{
                info!("LED Driver is not in Running state, cannot run");
                return;
            }
            let mut pulse_codes: [PulseCode; 24] = [LED_RET; 24];
            match self.function{
                LED_Function::Static => {
                    let pulse_codes = rgb_to_pulsecode(&self.color);
                    self.channel.transmit(&pulse_codes).unwrap();
                },
                LED_Function::Dimmed(dim_level) => {
                    let dimmed_color = RGB{
                        r: (self.color.r as f32 * (dim_level as f32 / 255.0)) as u8,
                        g: (self.color.g as f32 * (dim_level as f32 / 255.0)) as u8,
                        b: (self.color.b as f32 * (dim_level as f32 / 255.0)) as u8,
                    };
                    let pulse_codes = rgb_to_pulsecode(&dimmed_color);
                    self.channel.transmit(&pulse_codes).unwrap();
                },
                LED_Function::Breathing(breath_freq) => {
                },
                LED_Function::Off => {
                },

            }
        }

        fn rgb_to_pulsecode(rgb: &RGB) -> [PulseCode; 24] {
            let mut pulse_codes = [LED_RET; 24];
            for i in 0..8 {
                pulse_codes[i] = if (rgb.g & (1 << (7 - i))) != 0 { LED_1 } else { LED_0 };
                pulse_codes[i + 8] = if (rgb.r & (1 << (7 - i))) != 0 { LED_1 } else { LED_0 };
                pulse_codes[i + 16] = if (rgb.b & (1 << (7 - i))) != 0 { LED_1 } else { LED_0 };
            }
            pulse_codes
        }

        fn rgb_array_to_pulsecode(rgb_array: &[RGB], data: &mut [PulseCode]) {
            let mut index = 0;
            for rgb in rgb_array {
                let pulse_codes = rgb_to_pulsecode(rgb);
                for pulse_code in &pulse_codes {
                    data[index] = *pulse_code;
                    index += 1;
                }
            }
            data[index] = LED_RET; // Add the end marker
        }

        fn dim_color(rgb: &mut RGB, factor: f32) {
            rgb.r = (rgb.r as f32 * factor).min(255.0) as u8;
            rgb.g = (rgb.g as f32 * factor).min(255.0) as u8;
            rgb.b = (rgb.b as f32 * factor).min(255.0) as u8;
        }
    }
}
// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 2.0.0
    // generator parameters: -o unstable-hal -o esp32c6 -o esp32c6-mini-1 -o probe-rs -o defmt -o panic-rtt-target -o cargo-embed -o embassy -o vscode -o stable-x86_64-unknown-linux-gnu

    rtt_target::rtt_init_defmt!();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);


    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0, peripherals.FROM_CPU_INTR0);

    info!("Embassy initialized!");


    // TODO: Spawn some tasks
    let _ = spawner;

    loop {
        info!("Hello world!");
        Timer::after(Duration::from_secs(1)).await;
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.2.2/examples
}
