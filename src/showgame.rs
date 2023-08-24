use crate::Direction;
use crate::TorusSnakeGame;
// use anyhow::Error;
use anyhow::Result;
// use std::fmt::Result;
use std::fs::File;
use std::io::{BufRead, BufReader};
// use std::path::Path;
use crate::parse_instruction::Instruction;
use std::time::Duration;

#[derive(Debug, PartialEq, thiserror::Error)]
enum ParseError {
    #[error("Wrong amount of arguments")]
    ArgNoErr,
    // #[error("Invalid move")]
    // MoveErr,
}

pub fn parse_usize_pair(input: &str) -> Result<(usize, usize)> {
    let parsed_args: Vec<usize> = input
        .split(",")
        .filter_map(|num_str| num_str.parse::<usize>().ok())
        .collect();

    let [x, y] = parsed_args[..] else {
        return Err(ParseError::ArgNoErr.into());  // note that due to filter_map, the number of 
        // args may actually be correct, but the number of successfully parsed args is not
    };
    Ok((x, y))
}

pub fn parse_player_move(input: &str) -> Result<(usize, Direction)> {
    let mut args = input.split(":");
    let player: usize = args.next().ok_or(ParseError::ArgNoErr)?.parse()?;
    let direction: Direction = args.next().ok_or(ParseError::ArgNoErr)?.parse()?;
    Ok((player, direction))
}

pub fn showgame(logfile: &str, timestep: u64) -> Result<()> {
    let file = File::open(logfile)?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let (width, height) = parse_usize_pair(&lines.next().unwrap()?)?;

    let n_players = lines.next().unwrap()?.parse::<usize>()?;
    let starting_positions: Vec<(usize, usize)> = (0..n_players)
        .map(|_| parse_usize_pair(&lines.next().unwrap().unwrap()).unwrap())
        .collect();

    let mut game = TorusSnakeGame::new(width, height, starting_positions);

    for line_result in lines {
        let Ok(line) = line_result else{continue;};
        use Instruction::*;
        match line.parse::<Instruction>()? {
            Move { player, direction } => {
                game.move_player(player, direction);

                println!("\n{player}:{direction}");
                println!("{}", game);
                std::thread::sleep(Duration::from_millis(timestep));
                // clear_lines(n_players + 1);
                print!("{}", term_cursor::Up(height as i32 + 4)); // move cursor up to overwrite previous board
                                                                  // print!("{}\r", "\x1B[F".to_owned().repeat(height + 4))
                                                                  // print!("\x1B[{}A\r", height + 4)
            }
            _ => continue,
        }
    }

    print!("{}", term_cursor::Down(height as i32 + 4)); // set cursor to below board when we're done
    println!(); // extra clear line for aesthetics
    Ok(())
}
