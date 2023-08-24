use colored::Colorize;
use itertools::Itertools;
use std::collections::HashSet;

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
        if let Some(player) = self.get(pos) {
            let disp = player.to_string();
            if self.head_positions[player] != pos {
                return disp;
            }
            let mut disp = disp.bold();
            if self.alive_players.contains(&player) {
                disp = disp.green()
            } else {
                disp = disp.red()
            }
            format!("{}", disp)
        } else {
            "·".into()
        }
    }

    #[allow(unstable_name_collisions)] // intersperse will be added to std, but change is probably not breaking
    pub fn setup_string(&self) -> String {
        format!(
            "{},{}\n{}\n{}",
            self.width,
            self.height,
            self.head_positions.len(),
            self.head_positions
                .iter()
                .map(|(x, y)| format!("{x},{y}"))
                .intersperse("\n".to_owned())
                .collect::<String>()
        )
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

#[derive(Debug, thiserror::Error)]
#[error("Could not be parsed to direction")]
pub struct InvalidDirection;

impl std::str::FromStr for Direction {
    type Err = InvalidDirection;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "n" | "north" => Ok(Self::North),
            "s" | "south " => Ok(Self::South),
            "e" | "east" => Ok(Self::East),
            "w" | "west" => Ok(Self::West),
            _ => Err(InvalidDirection {}),
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
