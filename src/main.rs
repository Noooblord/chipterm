mod core;
mod display;
mod utils;

use crate::core::Chip8;
use crate::utils::BUTTONMAP;
use display::Display;

use rand::Rng;
use signal_hook;
use signal_hook::consts::signal::SIGWINCH;
use std::{
    io,
    sync::mpsc::channel,
    thread,
    time::{Duration, Instant},
};
use termion::{
    event::Event,
    event::Key,
    input::TermRead,
    raw::{IntoRawMode, RawTerminal},
};
use tui::backend::TermionBackend;
use tui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use tui::style::Color;
use tui::text::{Span, Spans};
use tui::widgets::{canvas::Canvas, Block, Borders, Paragraph, Wrap};
use tui::Terminal;
use tui::{style::Style, symbols};

#[derive(Debug, Clone)]
pub struct App {
    pub debug: bool,
    pub show_real_controls: bool,
    pub rewind: u8,
    pub paused: bool,
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

fn main() -> Result<(), io::Error> {
    let mut app = App::new();
    let mut chip8 = Chip8::new();
    let romdata = std::fs::read("./logo.ch8")?;
    chip8.load_game(&romdata)?;

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
        delay_timer_tick_tx.send(Event::Key(Key::F(13)));
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
                chip8.load_game(&romdata)?;
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

        let (registers, controls) = (chunks[0], chunks[1]);

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

        let (help, stack) = (chunks[0], chunks[1]);

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
    if chip8.keys[key.to_digit(16).unwrap() as usize] > 0 {
        widget = widget.style(Style::default().fg(Color::Red));
    }
    widget
}
