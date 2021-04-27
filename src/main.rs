use lazy_static::lazy_static;
use rand::Rng;
use signal_hook;
use signal_hook::consts::signal::SIGWINCH;
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::{
    io,
    sync::mpsc::channel,
    thread::{self, sleep},
    time::{Duration, Instant},
};
use termion::{
    event::Event,
    event::Key,
    input::TermRead,
    raw::{IntoRawMode, RawTerminal},
};
use tui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use tui::style::Color;
use tui::text::{Span, Spans};
use tui::widgets::{
    canvas::{Canvas, Line, Map, MapResolution, Rectangle, Shape},
    Block, Borders, Paragraph, Sparkline, Widget, Wrap,
};
use tui::Terminal;
use tui::{
    backend::{Backend, TermionBackend},
    Frame,
};
use tui::{style::Style, symbols};

// lazy_static! {
//     static ref PRIVILEGES: HashMap<&'static str, Vec<&'static str>> = {
//         let mut map = HashMap::new();
//         map.insert("James", vec!["user", "admin"]);
//         map.insert("Jim", vec!["user"]);
//         map
//     };
// }
lazy_static! {
    static ref BUTTONMAP: HashMap<char, &'static str> = {
        let mut map = HashMap::new();
        map.insert('0', "X");
        map.insert('1', "1");
        map.insert('2', "2");
        map.insert('3', "3");
        map.insert('4', "Q");
        map.insert('5', "W");
        map.insert('6', "E");
        map.insert('7', "A");
        map.insert('8', "S");
        map.insert('9', "D");
        map.insert('A', "Z");
        map.insert('B', "C");
        map.insert('C', "4");
        map.insert('D', "R");
        map.insert('E', "F");
        map.insert('F', "V");
        map
    };
}

lazy_static! {
    static ref CHARMAP: HashMap<char, u8> = {
        let mut map = HashMap::new();
        map.insert('0', 0x0u8);
        map.insert('1', 0x1u8);
        map.insert('2', 0x2u8);
        map.insert('3', 0x3u8);
        map.insert('4', 0x4u8);
        map.insert('5', 0x5u8);
        map.insert('6', 0x6u8);
        map.insert('7', 0x7u8);
        map.insert('8', 0x8u8);
        map.insert('9', 0x9u8);
        map.insert('A', 0xAu8);
        map.insert('B', 0xBu8);
        map.insert('C', 0xCu8);
        map.insert('D', 0xDu8);
        map.insert('E', 0xEu8);
        map.insert('F', 0xFu8);
        map
    };
}

#[derive(Debug, Clone)]
struct Display {
    grid: [[u8; 32]; 64],
}

impl Shape for Display {
    fn draw(&self, painter: &mut tui::widgets::canvas::Painter) {
        let max_y = 32;
        let max_x = 64;
        // let mut output = std::fs::File::create("screendbg").unwrap();
        for y in 0..max_y {
            for x in 0..max_x {
                let pixel = self.grid[x][y];
                // write!(output, "{}", pixel).unwrap();
                if pixel == 1 {
                    let (x, y) = painter.get_point(x as f64, (31 - y) as f64).unwrap();
                    painter.paint(x, y, Color::Red)
                }
            }
            // writeln!(output).unwrap();
        }
    }
}

impl Display {
    fn cls(&mut self) {
        self.grid.fill([0; 32]);
    }
    fn draw_sprite(
        &mut self,
        sprite_start_x: u8,
        sprite_start_y: u8,
        mut sprite: Vec<Vec<u8>>,
    ) -> bool {
        // let mut output = std::fs::File::create("spritedbg").unwrap();
        let max_y = sprite[0].len() - 1;
        let max_x = sprite.len() - 1;
        let mut collision = false;
        let sprite_start_x = sprite_start_x % 63;
        let sprite_start_y = sprite_start_y % 31;
        for y in 0..=max_y {
            for x in 0..=max_x {
                // write!(output, "{}", sprite[x][y]).unwrap();
                let x_coord = sprite_start_x as usize + x;
                let y_coord = sprite_start_y as usize + y;
                if !(x_coord < 64 && y_coord < 32) {
                    continue;
                }
                let pixel = self.grid[x_coord][y_coord];
                if pixel == 1 && sprite[x][y] == 1 {
                    collision = true;
                }
                self.grid[x_coord][y_coord] ^= sprite[x][y];
            }
            // writeln!(output).unwrap();
        }
        // writeln!(
        //     output,
        //     "Sprite location - x:{} y:{}",
        //     sprite_start_x, sprite_start_y
        // )
        // .unwrap();
        // writeln!(output, "Sprite max - x:{} y:{}", max_x, max_y).unwrap();
        collision
    }
}

#[derive(Debug, Clone)]
struct App {
    debug: bool,
    show_real_controls: bool,
    rewind: u8,
    paused: bool,
}

impl App {
    fn new() -> Self {
        App {
            debug: false,
            show_real_controls: true,
            rewind: 0,
            paused: false,
        }
    }
}

#[derive(Debug, Clone)]
struct Chip8 {
    opcode: u16,
    program_counter: u16,
    mem: [u8; 4096],
    vreg: [u8; 16],
    ireg: u16,
    gfx: Display,
    delay_timer: u8,
    stack: [u16; 16],
    stack_pointer: u16,
    keys: [u8; 16],
    desc: String,
}

impl Chip8 {
    fn new() -> Self {
        let mut new = Chip8 {
            opcode: 0,
            program_counter: 512,
            mem: [0; 4096],
            vreg: [0; 16],
            ireg: 0,
            gfx: Display {
                grid: [[0u8; 32]; 64],
            },
            delay_timer: 0,
            stack: [0; 16],
            stack_pointer: 0,
            keys: [0; 16],
            desc: String::from(""),
        };
        new.load_fonts();
        new
    }

    fn press_key(&mut self, key: u8) {
        self.keys.fill(0);
        self.keys[key as usize] = 2;
    }

    fn load_fonts(&mut self) {
        let mut fonts = [
            0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
            0x20, 0x60, 0x20, 0x20, 0x70, // 1
            0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
            0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
            0x90, 0x90, 0xF0, 0x10, 0x10, // 4
            0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
            0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
            0xF0, 0x10, 0x20, 0x40, 0x40, // 7
            0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
            0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
            0xF0, 0x90, 0xF0, 0x90, 0x90, // A
            0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
            0xF0, 0x80, 0x80, 0x80, 0xF0, // C
            0xE0, 0x90, 0x90, 0x90, 0xE0, // D
            0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
            0xF0, 0x80, 0xF0, 0x80, 0x80, // F
        ]
        .iter();
        for i in 0x050..=0x09F {
            self.mem[i] = *fonts.next().unwrap();
        }
    }
    fn load_game(&mut self, path: &str) -> Result<(), io::Error> {
        let mut memptr = 0x200;
        let romdata = std::fs::read("./logo.ch8")?;
        // let len = romdata.len() + 0x200;
        for data in romdata {
            self.mem[memptr] = data;
            memptr += 1;
        }
        Ok(())
    }
    fn decrement_delay_timer(&mut self) {
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
        }
    }
    fn emulation_cycle(&mut self) {
        // Fetch opcode from memory
        let opcode = (self.mem[self.program_counter as usize] as u16) << 8
            | self.mem[(self.program_counter + 1) as usize] as u16;
        self.opcode = opcode;
        // Increment program counter
        self.program_counter += 2;

        // Decode opcode

        // First "nibble" aka instruction
        let instruction = ((opcode & 0xF000) >> 12) as u8;

        // Second "nibble"
        let x = ((opcode & 0x0F00) >> 8) as u8;

        // Third "nibble"
        let y = ((opcode & 0x00F0) >> 4) as u8;

        // Fourth "nibble" and constants
        let n = (opcode & 0x000F) as u8;
        let nn = (opcode & 0x00FF) as u8;
        let nnn = opcode & 0x0FFF;
        match instruction {
            0 => match nn {
                //00EE	Flow	return;	    Returns from a subroutine.
                0xEE => {
                    self.desc = format!(
                        "[{:#X}] opcode\nFlow return. \
                        Returns from subroutine. Decrement stack size from {} to {}. \
                        Set program counter to previous stack value",
                        instruction,
                        self.stack_pointer,
                        self.stack_pointer - 1
                    );
                    self.stack_pointer -= 1;
                    self.program_counter = self.stack[self.stack_pointer as usize];
                }
                //00E0	Display	clear()	    Clears the screen.
                0xE0 => {
                    self.gfx.cls();
                    self.desc = format!(
                        "[{:#X}] opcode\nClear display. Fills screen with 0's.",
                        instruction
                    )
                }
                //0NNN	Call	            Calls machine code routine (RCA 1802 for COSMAC VIP) at address NNN. Not necessary for most ROMs.
                _ => {
                    self.stack[self.stack_pointer as usize] = self.program_counter;
                    self.stack_pointer += 1;
                    self.program_counter = nnn;
                }
            },
            //1NNN	Flow	goto NNN;	Jumps to address NNN.
            1 => {
                self.program_counter = nnn;
            }
            //2NNN	Flow	*(0xNNN)()	Calls subroutine at NNN.
            2 => {
                self.stack[self.stack_pointer as usize] = self.program_counter;
                self.stack_pointer += 1;
                self.program_counter = nnn;
            }
            //3XNN	Cond	if(Vx==NN)	Skips the next instruction if VX equals NN. (Usually the next instruction is a jump to skip a code block)
            3 => {
                unimplemented!()
            }
            //4XNN	Cond	if(Vx!=NN)	Skips the next instruction if VX does not equal NN. (Usually the next instruction is a jump to skip a code block)
            4 => {
                unimplemented!()
            }
            //5XY0	Cond	if(Vx==Vy)	Skips the next instruction if VX equals VY. (Usually the next instruction is a jump to skip a code block)
            5 => {
                unimplemented!()
            }
            //6XNN	Const	Vx = NN	    Sets VX to NN.
            6 => self.vreg[x as usize] = nn,
            //7XNN	Const	Vx += NN	Adds NN to VX. (Carry flag is not changed)
            7 => self.vreg[x as usize] += nn,
            //8XY0	Assign	Vx=Vy	    Sets VX to the value of VY.
            8 => match n {
                //8XY1	BitOp	Vx=Vx|Vy	Sets VX to VX or VY. (Bitwise OR operation)
                1 => {
                    unimplemented!()
                }
                //8XY2	BitOp	Vx=Vx&Vy	Sets VX to VX and VY. (Bitwise AND operation)
                2 => {
                    unimplemented!()
                }
                //8XY3	BitOp	Vx=Vx^Vy	Sets VX to VX xor VY.
                3 => {
                    unimplemented!()
                }
                //8XY4	Math	Vx += Vy	Adds VY to VX. VF is set to 1 when there's a carry, and to 0 when there is not.
                4 => {
                    unimplemented!()
                }
                //8XY5	Math	Vx -= Vy	VY is subtracted from VX. VF is set to 0 when there's a borrow, and 1 when there is not.
                5 => {
                    unimplemented!()
                }
                //8XY6	BitOp	Vx>>=1	    Stores the least significant bit of VX in VF and then shifts VX to the right by 1.[b]
                6 => {
                    unimplemented!()
                }
                //8XY7	Math	Vx=Vy-Vx	Sets VX to VY minus VX. VF is set to 0 when there's a borrow, and 1 when there is not.
                7 => {
                    unimplemented!()
                }
                //8XYE	BitOp	Vx<<=1	    Stores the most significant bit of VX in VF and then shifts VX to the left by 1.[b]
                _ => {
                    unimplemented!()
                }
            },
            //9XY0	Cond	if(Vx!=Vy)	Skips the next instruction if VX does not equal VY. (Usually the next instruction is a jump to skip a code block)
            9 => {
                unimplemented!()
            }
            //ANNN	MEM	    I = NNN	    Sets I to the address NNN.
            0xA => self.ireg = nnn,
            //BNNN	Flow	PC=V0+NNN	Jumps to the address NNN plus V0.
            0xB => {
                unimplemented!()
            }
            //CXNN	Rand	Vx=rand()&NN	Sets VX to the result of a bitwise and operation on a random number (Typically: 0 to 255) and NN.
            0xC => {
                unimplemented!()
            }
            //DXYN	Disp	draw(Vx,Vy,N)
            //Draws a sprite at coordinate (VX, VY) that has a width of 8 pixels and a height of N+1 pixels.
            //Each row of 8 pixels is read as bit-coded starting from memory location I;
            //I value does not change after the execution of this instruction.
            //As described above, VF is set to 1 if any screen pixels are flipped from set to unset
            //when the sprite is drawn, and to 0 if that does not happen.
            0xD => {
                let x_coord = self.vreg[x as usize];
                let y_coord = self.vreg[y as usize];
                // for each 8 pixels array in memory
                let mut sprite = vec![vec![0u8; n as usize]; 8];
                for (index, row) in self.mem[self.ireg as usize..self.ireg as usize + n as usize]
                    .iter()
                    .enumerate()
                {
                    // for each pixel in array
                    for i in 0..8 {
                        sprite[7 - i][index] = (row >> i) & 0b00000001
                    }
                }
                // let mut sprite = vec![vec![0u8; n as usize + 1]; 8];
                // for (index, row) in self.mem[self.ireg as usize..(self.ireg + n as u16) as usize]
                //     .into_iter()
                //     .enumerate()
                // {
                //     // for each pixel in array
                //     for i in 0..8 {
                //         sprite[0 + i][index] = (row >> i) & 0b0001
                //     }
                // }
                self.vreg[0xF] = self.gfx.draw_sprite(x_coord, y_coord, sprite) as u8;
            }
            0xE => match nn {
                //EX9E	KeyOp	if(key()==Vx)	Skips the next instruction if the key stored in VX is pressed. (Usually the next instruction is a jump to skip a code block)
                0x9E => {
                    unimplemented!()
                }
                //EXA1	KeyOp	if(key()!=Vx)	Skips the next instruction if the key stored in VX is not pressed. (Usually the next instruction is a jump to skip a code block)
                _ => {
                    unimplemented!()
                }
            },
            0xF => match nn {
                //FX07	Timer	Vx = get_delay()	Sets VX to the value of the delay timer.
                7 => {
                    unimplemented!()
                }
                //FX0A	KeyOp	Vx = get_key()	A key press is awaited, and then stored in VX. (Blocking Operation. All instruction halted until next key event)
                0xA => {
                    unimplemented!()
                }
                //FX15	Timer	delay_timer(Vx)	Sets the delay timer to VX.
                0x15 => {
                    unimplemented!()
                }
                //FX18	Sound	sound_timer(Vx)	Sets the sound timer to VX.
                0x18 => {
                    unimplemented!()
                }
                //FX1E	MEM	    I +=Vx	Adds VX to I. VF is not affected.[c]
                0x1E => {
                    unimplemented!()
                }
                //FX29	MEM	    I=sprite_addr[Vx]	Sets I to the location of the sprite for the character in VX. Characters 0-F (in hexadecimal) are represented by a 4x5 font.
                0x29 => {
                    unimplemented!()
                }
                //FX33	BCD	    Stores the binary-coded decimal representation of VX, with the most significant of three digits at the address in I, the middle digit at I plus 1, and the least significant digit at I plus 2. (In other words, take the decimal representation of VX, place the hundreds digit in memory at location in I, the tens digit at location I+1, and the ones digit at location I+2.)
                0x33 => {
                    //(251 / 10) % 10)
                    let num = self.vreg[x as usize];
                    self.mem[self.ireg as usize] = (num / 100) % 10;
                    self.mem[self.ireg as usize + 1] = (num / 10) % 10;
                    self.mem[self.ireg as usize + 2] = num % 10;
                }
                //FX55	MEM	    reg_dump(Vx,&I)	Stores V0 to VX (including VX) in memory starting at address I. The offset from I is increased by 1 for each value written, but I itself is left unmodified.[d]
                0x55 => {
                    unimplemented!()
                }
                //FX65	MEM	    reg_load(Vx,&I)	Fills V0 to VX (including VX) with values from memory starting at address I. The offset from I is increased by 1 for each value written, but I itself is left unmodified.[d]
                _ => {
                    unimplemented!()
                }
            },
            _ => panic!("Should never happen, Chip8 dump: {:?}", self),
        }
    }
}

fn main() -> Result<(), io::Error> {
    let mut app = App::new();
    let mut chip8 = Chip8::new();

    chip8.load_game("logo.ch8")?;

    let stdin = io::stdin();
    let stdout = io::stdout().into_raw_mode()?;

    let (tx, rx) = channel();
    let input_tx = tx.clone();
    let signal_tx = tx.clone();
    let cpu_tick_tx = tx.clone();
    let delay_timer_tick_tx = tx.clone();
    let input_clear_tx = tx.clone();

    let mut terminal = Terminal::new(TermionBackend::new(stdout))?;
    let mut duration = std::time::Instant::now();

    terminal.clear()?;
    let mut emulation_state = vec![chip8.clone()];
    draw_frame(
        &mut terminal,
        &mut duration,
        &mut app,
        &chip8,
        &emulation_state,
    )?;

    // Input listener thread
    thread::spawn(move || {
        for event in stdin.events() {
            input_tx.send(event.unwrap()).unwrap();
        }
    });

    // Resize listener thread
    thread::spawn(move || {
        let mut signals = signal_hook::iterator::Signals::new(&[SIGWINCH]).unwrap();
        for _ in signals.forever() {
            signal_tx.send(Event::Key(Key::Ctrl('l'))).unwrap();
        }
    });

    // Cpu tick event (60Hz)
    thread::spawn(move || loop {
        thread::sleep(Duration::from_nanos(16666667));
        cpu_tick_tx.send(Event::Key(Key::Null)).unwrap();
    });

    // Timer tick event (60Hz)
    thread::spawn(move || loop {
        thread::sleep(Duration::from_nanos(16666667));
        delay_timer_tick_tx.send(Event::Key(Key::F(13))).unwrap();
    });

    thread::spawn(move || loop {
        thread::sleep(Duration::from_nanos(16666667 * 3));
        input_clear_tx.send(Event::Key(Key::F(14))).unwrap();
    });

    // Main loop for events processing
    for event in rx.iter() {
        match event {
            // ctrl keys
            Event::Key(Key::Ctrl('c')) => break,
            Event::Key(Key::Ctrl('d')) => app.debug = !app.debug,
            Event::Key(Key::Ctrl('o')) => app.show_real_controls = !app.show_real_controls,
            Event::Key(Key::Ctrl('r')) => {
                chip8 = Chip8::new();
                chip8.load_game("logo.ch8")?;
            }

            // contols
            Event::Key(Key::Char('1')) => chip8.press_key(0x1),
            Event::Key(Key::Char('2')) => chip8.press_key(0x2),
            Event::Key(Key::Char('3')) => chip8.press_key(0x3),
            Event::Key(Key::Char('4')) => chip8.press_key(0xC),
            Event::Key(Key::Char('q')) => chip8.press_key(0x4),
            Event::Key(Key::Char('w')) => chip8.press_key(0x5),
            Event::Key(Key::Char('e')) => chip8.press_key(0x6),
            Event::Key(Key::Char('r')) => chip8.press_key(0xD),
            Event::Key(Key::Char('a')) => chip8.press_key(0x7),
            Event::Key(Key::Char('s')) => chip8.press_key(0x8),
            Event::Key(Key::Char('d')) => chip8.press_key(0x9),
            Event::Key(Key::Char('f')) => chip8.press_key(0xE),
            Event::Key(Key::Char('z')) => chip8.press_key(0xA),
            Event::Key(Key::Char('x')) => chip8.press_key(0x0),
            Event::Key(Key::Char('c')) => chip8.press_key(0xB),
            Event::Key(Key::Char('v')) => chip8.press_key(0xF),

            Event::Key(Key::F(13)) => {
                chip8.decrement_delay_timer();
            }

            Event::Key(Key::Char('g')) => {
                app.rewind = 2;
            }

            Event::Key(Key::Char('<')) => {
                if emulation_state.len() > 0 {
                    chip8 = emulation_state.pop().unwrap()
                }
                draw_frame(
                    &mut terminal,
                    &mut duration,
                    &mut app,
                    &chip8,
                    &emulation_state,
                )?;
            }
            Event::Key(Key::Char('>')) => {
                emulation_state.push(chip8.clone());
                chip8.emulation_cycle();
                draw_frame(
                    &mut terminal,
                    &mut duration,
                    &mut app,
                    &chip8,
                    &emulation_state,
                )?;
            }

            Event::Key(Key::Char('p')) => app.paused = !app.paused,

            Event::Key(Key::F(14)) => {
                for key in chip8.keys.iter_mut() {
                    if key > &mut 0 {
                        *key -= 1;
                    }
                }
                if app.rewind > 0 {
                    app.rewind -= 1;
                }
            }

            // CPU timer tick
            Event::Key(Key::Null) => {
                // TODO decrement keyups (key is valid for two ticks)
                if app.rewind > 0 && emulation_state.len() > 0 {
                    chip8 = emulation_state.pop().unwrap();
                } else if !app.paused {
                    emulation_state.push(chip8.clone());
                    // Read from program counter and execute opcode
                    chip8.emulation_cycle();
                }
                // Draw canvas
                draw_frame(
                    &mut terminal,
                    &mut duration,
                    &mut app,
                    &chip8,
                    &emulation_state,
                )?
            }
            // Draw canvas
            _ => draw_frame(
                &mut terminal,
                &mut duration,
                &mut app,
                &chip8,
                &emulation_state,
            )?,
        }
    }
    Ok(())
}

fn draw_frame(
    term: &mut Terminal<TermionBackend<RawTerminal<io::Stdout>>>,
    duration: &mut Instant,
    app: &mut App,
    chip8: &Chip8,
    emulation_state: &Vec<Chip8>,
) -> Result<(), io::Error> {
    let frame_duration = duration.elapsed().as_millis();
    let cpu_cycles = emulation_state.len();
    let playback = if app.paused {
        "paused"
    } else if app.rewind > 0 {
        "<<"
    } else {
        ">"
    };

    term.draw(|f| {
        let size = f.size();
        if !app.debug {
            let block = Block::default()
                .title(format!(
                    "Chip8 Emulator [{} ms per frame][{} cpu cycles][{}]",
                    frame_duration, cpu_cycles, playback
                ))
                .borders(Borders::ALL);
            f.render_widget(block, size);
        }
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(0)
            .constraints(
                [
                    Constraint::Percentage(20),
                    Constraint::Percentage(60),
                    Constraint::Percentage(20),
                ]
                .as_ref(),
            )
            .split(f.size());

        let (col_left, col_middle, col_right) = (chunks[0], chunks[1], chunks[2]);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(col_left);

        let (registers, stack) = (chunks[0], chunks[1]);

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(1)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(registers);

        let (registers_left, registers_right) = (chunks[0], chunks[1]);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
            .split(col_middle);

        let (display, opcodeview) = (chunks[0], chunks[1]);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(col_right);

        let (help, controls) = (chunks[0], chunks[1]);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(controls);

        let (row1, row2, row3, row4) = (chunks[0], chunks[1], chunks[2], chunks[3]);

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(0)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(row1);
        let (pad1, pad2, pad3, padC) = (chunks[0], chunks[1], chunks[2], chunks[3]);
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(0)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(row2);
        let (pad4, pad5, pad6, padD) = (chunks[0], chunks[1], chunks[2], chunks[3]);
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(0)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(row3);
        let (pad7, pad8, pad9, padE) = (chunks[0], chunks[1], chunks[2], chunks[3]);
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(0)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(row4);
        let (padA, pad0, padB, padF) = (chunks[0], chunks[1], chunks[2], chunks[3]);

        if app.debug {
            let block = Block::default().title("CPU").borders(Borders::ALL);
            f.render_widget(block, registers);
            let text = Paragraph::new(vec![
                Spans::from(format!("OP: {:#X}", chip8.opcode)),
                Spans::from(format!("V0: {:#X}", chip8.vreg[0])),
                Spans::from(format!("V2: {:#X}", chip8.vreg[2])),
                Spans::from(format!("V4: {:#X}", chip8.vreg[4])),
                Spans::from(format!("V6: {:#X}", chip8.vreg[6])),
                Spans::from(format!("V8: {:#X}", chip8.vreg[8])),
                Spans::from(format!("VA: {:#X}", chip8.vreg[0xA])),
                Spans::from(format!("VC: {:#X}", chip8.vreg[0xC])),
                Spans::from(format!("VE: {:#X}", chip8.vreg[0xE])),
                Spans::from(format!("IR: {:#X}", chip8.ireg)),
            ])
            .wrap(Wrap { trim: true });
            f.render_widget(text, registers_left);
            let text = Paragraph::new(vec![
                Spans::from(format!("PC: {:#X}", chip8.program_counter)),
                Spans::from(format!("V1: {:#X}", chip8.vreg[1])),
                Spans::from(format!("V3: {:#X}", chip8.vreg[3])),
                Spans::from(format!("V5: {:#X}", chip8.vreg[5])),
                Spans::from(format!("V7: {:#X}", chip8.vreg[7])),
                Spans::from(format!("V9: {:#X}", chip8.vreg[9])),
                Spans::from(format!("VB: {:#X}", chip8.vreg[0xB])),
                Spans::from(format!("VD: {:#X}", chip8.vreg[0xD])),
                Spans::from(format!("VF: {:#X}", chip8.vreg[0xF])),
                Spans::from(format!("DT: {:#X}", chip8.delay_timer)),
            ])
            .wrap(Wrap { trim: true });
            f.render_widget(text, registers_right);
            let block = Block::default().title("Stack").borders(Borders::ALL);
            f.render_widget(block, stack);
            f.render_widget(
                Block::default()
                    .title("Opcode description")
                    .borders(Borders::ALL),
                opcodeview,
            );
            let help_block = Block::default().title("Help").borders(Borders::ALL);
            let help_text = Paragraph::new(vec![
                Spans::from("ctrl+c -> exit emulator"),
                Spans::from("ctrl+d -> exit debug"),
                Spans::from("ctrl+o -> show original controls"),
            ])
            .wrap(Wrap { trim: true });
            f.render_widget(help_text.block(help_block), help);
            f.render_widget(
                Block::default().title("Controls").borders(Borders::ALL),
                controls,
            );
            f.render_widget(render_key_widget('1', app, chip8), pad1);
            f.render_widget(render_key_widget('2', app, chip8), pad2);
            f.render_widget(render_key_widget('3', app, chip8), pad3);
            f.render_widget(render_key_widget('C', app, chip8), padC);
            f.render_widget(render_key_widget('4', app, chip8), pad4);
            f.render_widget(render_key_widget('5', app, chip8), pad5);
            f.render_widget(render_key_widget('6', app, chip8), pad6);
            f.render_widget(render_key_widget('D', app, chip8), padD);
            f.render_widget(render_key_widget('7', app, chip8), pad7);
            f.render_widget(render_key_widget('8', app, chip8), pad8);
            f.render_widget(render_key_widget('9', app, chip8), pad9);
            f.render_widget(render_key_widget('E', app, chip8), padE);
            f.render_widget(render_key_widget('A', app, chip8), padA);
            f.render_widget(render_key_widget('0', app, chip8), pad0);
            f.render_widget(render_key_widget('B', app, chip8), padB);
            f.render_widget(render_key_widget('F', app, chip8), padF);
        }
        if size.height < 18 || size.width < 30 {
            let block = Block::default()
                .title("Small term size")
                .borders(Borders::ALL);
            let paragraph = Paragraph::new(vec![
                Spans::from(format!("{}x{} is too low!", size.width, size.height)),
                Spans::from("Chip8 display is 64×32 px"),
            ])
            .block(block)
            .wrap(Wrap { trim: true });
            f.render_widget(paragraph, centered_rect(60, 20, f.size()));
        } else {
            let canvas = Canvas::default()
                .marker(symbols::Marker::Block)
                .paint(|ctx| {
                    ctx.draw(&chip8.gfx);
                })
                .x_bounds([0.0, 64.0])
                .y_bounds([0.0, 32.0]);

            let canvas = if app.debug {
                canvas.block(Block::default().title("Display").borders(Borders::ALL))
            } else {
                canvas
            };

            f.render_widget(
                canvas,
                if app.debug {
                    display
                } else {
                    Rect {
                        x: 1,
                        y: 1,
                        height: size.height - 2,
                        width: size.width - 2,
                    }
                },
            )
        }
        *duration = Instant::now();
    })?;
    Ok(())
}

fn make_debug_layout() {}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}

fn render_key_widget<'a>(key: char, app: &'a App, chip8: &'a Chip8) -> Paragraph<'a> {
    let letter = if app.show_real_controls {
        key.to_string()
    } else {
        BUTTONMAP.get(&key).unwrap().to_string()
    };
    let mut widget = Paragraph::new(Span::raw(letter))
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
    if chip8.keys[*CHARMAP.get(&key).unwrap() as usize] > 0 {
        widget = widget.style(Style::default().fg(Color::Red));
    }
    widget
}
