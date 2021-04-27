use std::collections::HashMap;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref BUTTONMAP: HashMap<char, &'static str> = {
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
