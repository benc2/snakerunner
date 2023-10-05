use std::{collections::HashSet, io, str::FromStr};
// use anyhow; // if you want proper error handling, you can do it yourself. I don't get paid for this shit

// struct to keep track of the game
pub struct TorusSnakeGame {
    board: Vec<Vec<Option<usize>>>,
    height: usize,
    width: usize,
    head_positions: Vec<(usize, usize)>,
    alive_players: HashSet<usize>,
}

impl TorusSnakeGame {
    pub fn new(width: usize, height: usize, starting_positions: Vec<(usize, usize)>) -> Self {
        let mut board = vec![vec![None; width]; height];
        for (player_num, (x, y)) in starting_positions.iter().enumerate() {
            board[*y][*x] = Some(player_num);
        }
        let n_players = starting_positions.len();
        Self {
            board,
            height,
            width,
            head_positions: starting_positions,
            alive_players: (0..n_players).collect(),
        }
    }

    pub fn shift_coords(&self, (x, y): (usize, usize), direction: Direction) -> (usize, usize) {
        let (dx, dy) = direction.coord_shift();
        let new_x = ((x + self.width) as i32 + dx) as usize % self.width;
        let new_y = ((y + self.height) as i32 + dy) as usize % self.height;
        (new_x, new_y)
    }

    pub fn get(&self, (x, y): (usize, usize)) -> Option<usize> {
        self.board[y][x]
    }

    // fn clear(&mut self, (x, y): (usize, usize)) {
    //     self.board[y][x] = None;
    // }

    pub fn set_player(&mut self, (x, y): (usize, usize), player: usize) {
        self.board[y][x] = Some(player);
    }

    pub fn move_player(&mut self, player: usize, direction: Direction) -> bool {
        let new_pos = self.shift_coords(self.head_positions[player], direction);
        if self.get(new_pos).is_none() {
            self.head_positions[player] = new_pos;
            self.set_player(new_pos, player);
            true
        } else {
            self.alive_players.remove(&player);
            false
        }
    }

    pub fn display_cell(&self, pos: (usize, usize)) -> String {
        match self.get(pos) {
            Some(player) => player.to_string(),
            None => "·".into(),
        }
    }
}

impl std::fmt::Display for TorusSnakeGame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let horizontal_border = format!("+{}+", "-".to_owned().repeat(self.width));
        write!(
            f,
            "{}\n{}{}",
            horizontal_border.clone(),
            (0..self.height)
                .map(|y| format!(
                    "|{}|\n",
                    (0..self.width)
                        .map(|x| self.display_cell((x, y)))
                        .collect::<String>()
                ))
                .collect::<String>(),
            horizontal_border
        )
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Direction {
    North,
    South,
    East,
    West,
}

impl Direction {
    fn coord_shift(&self) -> (i32, i32) {
        match self {
            Self::North => (0, -1),
            Self::South => (0, 1),
            Self::East => (1, 0),
            Self::West => (-1, 0),
        }
    }
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::North => "N",
                Self::South => "S",
                Self::East => "E",
                Self::West => "W",
            }
            .to_string()
        )
    }
}

// stuff to parse inputs

impl std::str::FromStr for Direction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "N" => Ok(Self::North),
            "S" => Ok(Self::South),
            "E" => Ok(Self::East),
            "W" => Ok(Self::West),
            _ => Err("Could not parse direction".into()),
        }
    }
}

pub fn parse_player_move(input: &str) -> Result<(usize, Direction), String> {
    let mut args = input.split(":");
    let player: usize = args
        .next()
        .ok_or("Not enough arguments".to_string())?
        .parse()
        .map_err(|_| "Could not parse player number".to_string())?;
    let direction: Direction = args
        .next()
        .ok_or("Not enough arguments".to_string())?
        .parse()?;
    Ok((player, direction))
}

pub fn parse_usize_pair(input: &str) -> Result<(usize, usize), String> {
    let parsed_args: Vec<usize> = input
        .split(",")
        .filter_map(|num_str| num_str.parse::<usize>().ok())
        .collect();

    let [x, y] = parsed_args[..] else {
        return Err("Could not parse".into());
    };
    Ok((x, y))
}

#[derive(Debug, PartialEq)]
pub enum Instruction {
    AskMove,
    PlayerMove { player: usize, direction: Direction },
    Out { player: usize },
    Stop,
}

// #[derive(Debug, thiserror::Error, PartialEq)]
// #[error("Could not be parsed to instruction")]
// pub struct InstructionParseError {}

impl FromStr for Instruction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use Instruction::*;
        match s {
            "stop" => Ok(Stop),
            "move" => Ok(AskMove),
            instr if &instr[..3] == "out" => {
                // if instruction starts with "out"
                let player = *&instr[4..]
                    .parse::<usize>()
                    .map_err(|_| "Could not parse player number".to_string())?;
                Ok(Out { player })
            }
            instr => {
                let (player, direction) = parse_player_move(instr)?;
                Ok(PlayerMove { player, direction })
            }
        }
    }
}

fn main() {
    let mut lines = io::stdin().lines();
    let (width, height) = parse_usize_pair(&lines.next().unwrap().unwrap()).unwrap();
    let n_players: usize = lines.next().unwrap().unwrap().parse().unwrap();
    let starting_positions: Vec<(usize, usize)> = (0..n_players)
        .map(|_| parse_usize_pair(&lines.next().unwrap().unwrap()).unwrap())
        .collect();
    let my_player_number: usize = lines.next().unwrap().unwrap().parse().unwrap();
    let mut game = TorusSnakeGame::new(width, height, starting_positions);

    for line in lines {
        match line.unwrap().parse::<Instruction>().unwrap() {
            Instruction::Stop => break,
            Instruction::AskMove => {
                use Direction as D;
                let mut my_move = D::North;
                for direction in [D::North, D::East, D::South, D::West] {
                    my_move = direction;
                    if game.move_player(my_player_number, direction) {
                        break;
                    }
                }
                println!("{}", my_move);
            }
            Instruction::Out { player } => {
                game.alive_players.remove(&player);
            }
            Instruction::PlayerMove { player, direction } => {
                game.move_player(player, direction);
            }
        }
    }
}
