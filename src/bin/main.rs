#![no_std]
#![no_main]


#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use esp_hal::rmt;
use esp_hal::{clock::CpuClock, peripherals::Peripherals};
use esp_hal::timer::timg::TimerGroup;


use esp_hal::gpio::Pin;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};

use defmt::info;

use panic_rtt_target as _;



mod LED_Driver{
    use defmt::info;
    use libm::sinf;
    use core::f32::consts::PI;
    use embassy_time::{Duration, Timer};
    use esp_hal::clock::CpuClock;
    use esp_hal::time::Rate;
    use esp_hal::gpio::{Level, AnyPin};
    use esp_hal::peripherals::{RMT};
    use esp_hal::rmt::{PulseCode, Rmt, TxChannelConfig, TxChannelCreator, Channel};
    const MAX_LED_COUNT: usize = 255;
    const LED_0: PulseCode = PulseCode::new(Level::High, 4, Level::Low, 9);
    const LED_1: PulseCode = PulseCode::new(Level::High, 8, Level::Low, 5);
    const LED_RET: PulseCode = PulseCode::end_marker();
    #[derive(Clone, Copy, Debug)]
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
        Breathing(f32),
        Off,
    }

    pub struct LED_Driver<'a>{
        state: LED_Driver_State,
        function: LED_Function,
        tx_config: TxChannelConfig,
        channel: Option<esp_hal::rmt::Channel<'a, esp_hal::Blocking, esp_hal::rmt::Tx>>,
        colors: [RGB; MAX_LED_COUNT],
        led_count: usize,
    }

    impl<'a> LED_Driver<'a>{
        pub fn new(led_pin: AnyPin<'a>, per_rmt: RMT<'a>) -> Self{
            let rmt: Rmt<'_, esp_hal::Blocking>= Rmt::new(per_rmt, Rate::from_mhz(80)).unwrap();

            let tx_config = TxChannelConfig::default().with_clk_divider(8);

            let mut channel: esp_hal::rmt::Channel<'_, esp_hal::Blocking, esp_hal::rmt::Tx> = rmt
                .channel0
                .configure_tx(&tx_config).unwrap()
                .with_pin(led_pin);
            LED_Driver{
                state: LED_Driver_State::Initialized,
                function: LED_Function::Off,
                tx_config: tx_config,
                channel: Some(channel),
                colors: [RGB{r:0,g:0,b:0}; MAX_LED_COUNT],
                led_count: 0,
            }
        }

        pub fn set_color(&mut self, color: RGB, led_index: usize){
            self.colors[led_index] = color;
        }

        pub fn set_function(&mut self, function: LED_Function){
            self.function = function;
        }

        pub fn set_function_Dimmed(&mut self, dim_level: u8){
            self.function = LED_Function::Dimmed(dim_level);
        }

        pub fn set_function_Breathing(&mut self, breath_freq: f32){
            self.function = LED_Function::Breathing(breath_freq);
        }

        pub fn set_function_Off(&mut self){
            self.function = LED_Function::Off;
        }

        pub fn set_function_Static(&mut self){
            self.function = LED_Function::Static;
        }

        pub fn start(&mut self){
            match self.state{
                LED_Driver_State::Created => {
                    info!("LED Driver is in Created state, cannot start");
                    return;
                },
                LED_Driver_State::Running => {
                    info!("LED Driver is already running, cannot start");
                    return;
                },
                _ => {self.state = LED_Driver_State::Running;}
            }
        }

        pub fn stop(&mut self){
            match self.state{
                LED_Driver_State::Created => {
                    info!("LED Driver is in Created state, cannot stop");
                    return;
                },
                LED_Driver_State::Stopped => {
                    info!("LED Driver is already stopped, cannot stop");
                    return;
                },
                _ => {
                    self.state = LED_Driver_State::Stopped;
                }
            }
        }
        pub fn add_led(&mut self, color: RGB){
            if self.led_count < MAX_LED_COUNT{
                self.colors[self.led_count] = color;
                self.led_count += 1;
            }else{
                info!("Maximum LED count reached, cannot add more LEDs");
            }
        }

        pub fn remove_led(&mut self, index: usize){
            if index < self.led_count{
                for i in index..self.led_count-1{
                    self.colors[i] = self.colors[i+1];
                }
                self.colors[self.led_count-1] = RGB{r:0,g:0,b:0};
                self.led_count -= 1;
            }else{
                info!("LED index out of bounds, cannot remove LED");
            }
        }

        pub fn clear_leds(&mut self){
            for i in 0..self.led_count{
                self.colors[i] = RGB{r:0,g:0,b:0};
            }
            self.led_count = 0;
        }

        pub async fn run(&mut self){
            let mut time: f32 = 0.0;
            let time_step: f32 = 0.01; // Adjust this value to control the speed of the breathing effect
            let mut output_colors: [RGB; MAX_LED_COUNT] = [RGB { r: 0, g: 0, b: 0 }; MAX_LED_COUNT];
            let mut pulse_codes: [PulseCode; MAX_LED_COUNT*24 +1] = [LED_0; MAX_LED_COUNT*24 +1];
            loop{
                match self.state{
                    LED_Driver_State::Running => {
                        match self.function{
                            LED_Function::Static => {
                                output_colors = self.colors;
                            },
                            LED_Function::Dimmed(dim_level) => {
                            },
                            LED_Function::Breathing(breath_freq) => {
                                time = (time + time_step) % breath_freq;
                                //info!("Breathing effect time: {}", time);
                                Self::breathing_effect(&self.colors, &mut output_colors, time, breath_freq);
                            },
                            LED_Function::Off => {
                                output_colors = [RGB { r: 0, g: 0, b: 0 }; MAX_LED_COUNT];
                            },

                        }
                        Self::rgb_array_to_pulsecode(self,&output_colors, &mut pulse_codes);

                        let channel = self.channel.take().unwrap();
                        info!("Transmitting pulse codes for {} LEDs", self.led_count);
                        info!("Pulse codes: {:?}", &pulse_codes[..self.led_count * 24 + 1]);
                        //info!("Color values: {:?}", &output_colors[..self.led_count]);
                        //info!("Original color values: {:?}", &self.colors[..self.led_count]);
                        match channel.transmit(&pulse_codes[..self.led_count * 24 + 1]) {
                            Ok(transaction) => {
                                match transaction.wait() {
                                    Ok(_channel) => {
                                        self.channel = Some(_channel);
                                        //info!("RMT transmission completed successfully");
                                    }
                                    Err(error) => {
                                        info!("RMT transmission wait failed: {:?}", error);
                                        self.channel = Some(error.1);
                                    }
                                }
                            }
                            Err((error, _channel)) => {
                                info!("RMT transmission failed: {:?}", error);
                                self.channel = Some(_channel);
                            }
                        }
                        
                    },
                    _ => {
                        info!("LED Driver is not running, cannot run"); 
                    }
                }
            Timer::after(Duration::from_millis(10)).await;
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

        fn rgb_array_to_pulsecode(&mut self, rgb_array: &[RGB], data: &mut [PulseCode]) {
            let mut index = 0;
            for i in 0..self.led_count{
                let pulse_codes = Self::rgb_to_pulsecode(&rgb_array[i]);
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

        fn breathing_effect(
            original: &[RGB],
            output: &mut [RGB],
            time: f32,
            speed: f32,
        ) {
            let wave = sinf(time / speed * 2.0 * PI);

            let brightness = (wave + 1.0) * 0.5;

            let min_brightness = 0.05;
            let factor = min_brightness + brightness * (1.0 - min_brightness);

            for (src, dst) in original.iter().zip(output.iter_mut()) {
                dst.r = (src.r as f32 * factor) as u8;
                dst.g = (src.g as f32 * factor) as u8;
                dst.b = (src.b as f32 * factor) as u8;
            }
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
    let gpio15 = peripherals.GPIO15.degrade();
    let rmt: esp_hal::peripherals::RMT<'_> = peripherals.RMT;
    let mut led_driver = LED_Driver::LED_Driver::new(gpio15, rmt);
    for _ in 0..3 {
        led_driver.add_led(LED_Driver::LED_RED);
        led_driver.add_led(LED_Driver::LED_GREEN);
        led_driver.add_led(LED_Driver::LED_BLUE);
    }
    led_driver.add_led(LED_Driver::LED_YELLOW);
    led_driver.set_function_Breathing(2.0);
    led_driver.start();
    led_driver.run().await;
    let _ = spawner;

    loop {
        info!("Hello world!");
        Timer::after(Duration::from_secs(1)).await;
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.2.2/examples
}
