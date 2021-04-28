use std::io;

use crate::display::Display;

#[derive(Debug, Clone)]
pub struct Chip8 {
    pub opcode: u16,
    pub program_counter: u16,
    pub mem: [u8; 4096],
    pub vreg: [u8; 16],
    pub ireg: u16,
    pub gfx: Display,
    pub delay_timer: u8,
    pub stack: [u16; 16],
    pub stack_pointer: u16,
    pub keys: [u8; 16],
    pub desc: String,
}

impl Chip8 {
    pub fn new() -> Self {
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

    pub fn press_key(&mut self, key: u8) {
        self.keys.fill(0);
        self.keys[key as usize] = 2;
    }

    pub fn load_fonts(&mut self) {
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
    pub fn load_game(&mut self, rom: &Vec<u8>) -> Result<(), io::Error> {
        let mut memptr = 0x200;
        // let len = romdata.len() + 0x200;
        for data in rom {
            self.mem[memptr] = *data;
            memptr += 1;
        }
        Ok(())
    }
    pub fn decrement_delay_timer(&mut self) {
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
        }
    }
    pub fn emulation_cycle(&mut self) {
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
