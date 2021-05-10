use tui::{style::Color, widgets::canvas::Shape};

#[derive(Debug, Clone)]
pub struct Display {
    pub grid: [[u8; 32]; 64],
}

impl Shape for Display {
    fn draw(&self, painter: &mut tui::widgets::canvas::Painter) {
        let max_y = 32;
        let max_x = 64;
        for y in 0..max_y {
            for x in 0..max_x {
                let pixel = self.grid[x][y];
                if pixel == 1 {
                    let (x, y) = painter.get_point(x as f64, (31 - y) as f64).unwrap();
                    painter.paint(x, y, Color::Reset)
                }
            }
        }
    }
}

impl Display {
    pub fn cls(&mut self) {
        self.grid.fill([0; 32]);
    }
    pub fn draw_sprite(
        &mut self,
        sprite_start_x: u8,
        sprite_start_y: u8,
        sprite: Vec<Vec<u8>>,
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
