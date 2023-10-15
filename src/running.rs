use anyhow::Result;
use itertools::Itertools;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, LineWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc;
// use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use rand::distributions::{Distribution, Uniform};
use rand::seq::SliceRandom;
use rand::thread_rng;

use crate::game::{Direction, TorusSnakeGame};

enum Message {
    CommunicateMove { direction: Direction, player: usize },
    AskMove(usize), // ask player to move
    Kill(usize),
    SendHeader(String),
}

fn make_process_python(program_name: &str) -> Child {
    let python_command = if cfg!(windows) { "python" } else { "python3" };

    let path = Path::new(program_name);
    let mut dir = path.parent().unwrap_or(Path::new("./"));
    if dir == Path::new("") {
        // if no parent directory
        dir = Path::new("./");
    }
    let filename = path.file_name().unwrap();
    Command::new(python_command)
        .current_dir(dir)
        .arg("-m")
        .arg(filename)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap()
}

fn make_process_exe(program_name: &str) -> Child {
    Command::new(format!(r#"./{program_name}"#))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap()
}

fn make_process(filename: &str) -> Child {
    if let Some(program_name) = filename.strip_suffix(".py") {
        make_process_python(program_name)
    } else {
        make_process_exe(filename)
    }

    // else if let Some(program_name) = filename.strip_suffix(".exe") {
    //     make_process_exe(program_name)
    // } else {
    //     panic!()
    // }
}

fn write_to_player(
    message: &str,
    player: usize,
    stdin: &mut ChildStdin,
    _sender: &mpsc::Sender<usize>,
    alive_players: &mut HashSet<usize>,
    verbose: bool,
) {
    if verbose {
        println!("->p{player}  \"{message}\"")
    }
    stdin
        .write_all(format!("{message}\n").as_bytes())
        .unwrap_or_else(|_| {
            alive_players.remove(&player);
            // let _ = sender.send(player); // Not much we can do if this fails
        })
}

fn log(message: &str, writer_opt: &mut Option<LineWriter<File>>) -> Result<(), std::io::Error> {
    if let Some(writer) = writer_opt {
        writer.write_fmt(format_args!("{message}\n"))
    } else {
        Ok(())
    }
}

fn random_starting_positions(width: usize, height: usize, n_players: usize) -> Vec<(usize, usize)> {
    let mut starting_positions = Vec::new();
    let x_sampler = Uniform::new(0, width);
    let y_sampler = Uniform::new(0, height);
    while starting_positions.len() < n_players {
        // TODO if n_players too high, this loop can run forever
        let start_pos = (
            x_sampler.sample(&mut rand::thread_rng()),
            y_sampler.sample(&mut rand::thread_rng()),
        );
        if !starting_positions.contains(&start_pos) {
            starting_positions.push(start_pos);
        }
    }
    starting_positions
}

#[derive(Clone)]
pub enum LossReason {
    LosingMove,
    TimeOut,
    InvalidInput,
}

#[derive(Clone)]
pub enum PlayerResult {
    Winner,
    Loser(LossReason),
}

impl std::convert::From<PlayerStatus> for PlayerResult {
    fn from(value: PlayerStatus) -> Self {
        match value {
            PlayerStatus::Alive => Self::Winner,
            PlayerStatus::Dead(reason) => Self::Loser(reason),
        }
    }
}

impl PlayerResult {
    fn is_winner(&self) -> bool {
        match self {
            Self::Winner => true,
            _ => false,
        }
    }
}

#[derive(Clone)]
pub enum PlayerStatus {
    Alive,
    Dead(LossReason),
}

impl PlayerStatus {
    fn is_alive(&self) -> bool {
        match self {
            Self::Alive => true,
            _ => false,
        }
    }
}

pub fn play_game(
    scripts: &Vec<&str>,
    starting_config: Option<Vec<(usize, usize)>>,
    width: usize,
    height: usize,
    log_filename: Option<&Path>,
    verbose: bool,
    time_limit: u64,
) -> Option<(usize, Vec<PlayerResult>)> {
    // TODO make output a result
    let logfile = log_filename.map(|filename| File::create(filename).unwrap());
    let writer = logfile.map(LineWriter::new);

    let n_players = scripts.len();

    let starting_positions =
        starting_config.unwrap_or_else(|| random_starting_positions(width, height, n_players));

    let mut player_statuses = vec![PlayerStatus::Alive; n_players];

    let mut game = TorusSnakeGame::new(width, height, starting_positions);

    let mut children: Vec<Child> = scripts
        .into_iter()
        .map(|script_name| make_process(script_name.as_ref()))
        .collect();
    let stdins: Vec<_> = children
        .iter_mut()
        .map(|child| child.stdin.take().expect("Child has no stdin"))
        .collect();

    let mut readers: Vec<BufReader<ChildStdout>> = children
        .iter_mut()
        .map(|child| BufReader::new(child.stdout.take().expect("Child has no stdout")))
        .collect();

    let (read_sender, read_receiver) = mpsc::channel();
    let (write_sender, _write_receiver) = mpsc::channel(); // for now just used to kill child scripts

    // thread for writing IO
    thread::spawn(move || {
        writing_process(
            n_players,
            read_receiver,
            writer,
            stdins,
            write_sender,
            verbose,
        );
    });

    let (listener_sender, listener_receiver) = mpsc::channel(); // determines which child the thread will try to read from
    let (readline_sender, readline_receiver) = mpsc::channel();

    // thread for reading from IO
    thread::spawn(move || {
        for player in listener_receiver {
            // receives which player to read from
            let mut buffer = String::new();
            let reader: &mut BufReader<ChildStdout> = &mut readers[player]; // why do we need type annotation?
            let _ = reader.read_line(&mut buffer); //error handling? If it fails, it's probably players fault, so just return the empty buffer and let outer loop kill them
            let _ = readline_sender.send(buffer); // this appears to only fail when player times out
                                                  // apparently it still sends its output (even though the process is explicitly killed) but function has quit
                                                  // so channel is closed. Maybe .kill() is slow?
        }
    });

    // let mut attempts = 0;
    // let mut alive_players: HashSet<usize> = (0..n_players).collect();

    if verbose {
        println!("{}", game.setup_string());
    }
    read_sender
        .send(Message::SendHeader(game.setup_string()))
        .unwrap();

    let mut first_loop = true;
    'mainloop: loop {
        if verbose {
            println!(
                "\nRemaining players: {:?}",
                (0..n_players)
                    .filter(|player| player_statuses[*player].is_alive())
                    .collect_vec()
            );
        }
        for player in 0..n_players {
            // if alive_players.len() < 2 {
            if player_statuses
                .iter()
                .filter(|status| status.is_alive())
                .count()
                < 2
            {
                // condition to end the game
                // need to check here, since game can end after any move
                if verbose {
                    println!("done");
                }
                break 'mainloop;
            }

            // if !alive_players.contains(&player) {
            //     // skip dead players
            //     continue;
            // }

            if !player_statuses[player].is_alive() {
                // skip dead players
                continue;
            }

            read_sender.send(Message::AskMove(player)).unwrap();
            listener_sender.send(player).unwrap();
            let timeout_time = if first_loop {
                10 * time_limit
            } else {
                time_limit
            };

            // receive read messages from reading thread
            let line = match readline_receiver.recv_timeout(Duration::from_millis(timeout_time)) {
                Ok(line) => line,
                Err(_) => {
                    kill_player(
                        player,
                        &read_sender,
                        LossReason::TimeOut,
                        &mut player_statuses,
                    );
                    let _ = children[player].kill(); // TODO: maybe remove if we have a good plan for when to kill processes
                    println!("Timeout {}", player);
                    if verbose {
                        println!("Killing player {player} due to timeout");
                    }
                    continue;
                }
            };
            // .to_owned();

            // trim_newline(&mut line);
            if verbose {
                println!("<-p{player}  \"{}\"", line.trim());
            }
            match line.trim().parse() {
                Ok(direction) => {
                    if !game.move_player(player, direction) {
                        if verbose {
                            println!("Killing player {player} due to losing move");
                        }
                        kill_player(
                            player,
                            &read_sender,
                            LossReason::LosingMove,
                            &mut player_statuses,
                        );
                    }
                    read_sender
                        .send(Message::CommunicateMove { direction, player })
                        .unwrap();
                }
                Err(_) => {
                    // invalid move input from player
                    if verbose {
                        println!("Killing player {player} due to invalid input");
                    }
                    kill_player(
                        player,
                        &read_sender,
                        LossReason::InvalidInput,
                        &mut player_statuses,
                    );
                }
            }

            // TODO: should we kill a process when we kill the player?
            // TODO: kill players when they do not accept input
            // TODO: add time limit for players

            if verbose {
                println!("{}", game);
            }
            first_loop = false;
        }
    }
    for i in 0..n_players {
        if player_statuses[i].is_alive() {
            read_sender.send(Message::Kill(i)).unwrap(); // kill winning player
                                                         // TODO because kill_player takes a LossReason, we cannot use it to kill the winner.
                                                         // change kill_player, or leave as an exception like this?
        }
        let _ = children[i].kill();
    }
    // (winner, player_results)
    let player_results = player_statuses
        .into_iter()
        .map(PlayerResult::from)
        .collect_vec();
    let Some(winner) = player_results
        .iter()
        .position(PlayerResult::is_winner) else {
            return None;
        };

    Some((winner, player_results))
}

fn writing_process(
    n_players: usize,
    read_receiver: mpsc::Receiver<Message>,
    mut writer: Option<LineWriter<File>>,
    mut stdins: Vec<ChildStdin>,
    write_sender: mpsc::Sender<usize>,
    verbose: bool,
) {
    use Message as M;

    let mut alive_players: HashSet<usize> = (0..n_players).collect();
    for message in read_receiver.iter() {
        match message {
            M::CommunicateMove { direction, player } => {
                log(&format!("{player}:{direction}"), &mut writer).unwrap();

                for (opponent_player, stdin) in stdins.iter_mut().enumerate() {
                    if opponent_player == player || !alive_players.contains(&opponent_player) {
                        continue;
                    }

                    write_to_player(
                        &format!("{}:{}", player, direction),
                        opponent_player,
                        stdin,
                        &write_sender,
                        &mut alive_players,
                        verbose,
                    );
                }
            }

            M::AskMove(player) => {
                write_to_player(
                    "move",
                    player,
                    &mut stdins[player],
                    &write_sender,
                    &mut alive_players,
                    verbose,
                );
            }

            M::Kill(player) => {
                alive_players.remove(&player);
                write_to_player(
                    "stop",
                    player,
                    &mut stdins[player],
                    &write_sender,
                    &mut alive_players,
                    verbose,
                );

                // communicate death of player to others
                for (opponent_player, stdin) in stdins.iter_mut().enumerate() {
                    if opponent_player == player || !alive_players.contains(&opponent_player) {
                        // skip killed player
                        continue;
                    }

                    write_to_player(
                        &format!("out:{}", player),
                        opponent_player,
                        stdin,
                        &write_sender,
                        &mut alive_players,
                        verbose,
                    );
                }
            }

            M::SendHeader(header) => {
                for (player, stdin) in stdins.iter_mut().enumerate() {
                    write_to_player(
                        &format!("{header}\n{player}"),
                        player,
                        stdin,
                        &write_sender,
                        &mut alive_players,
                        verbose,
                    );
                }
                log(&header, &mut writer).unwrap();
            }
        }
    }
}

fn log_summary(writer: &mut LineWriter<File>, wins: Vec<i32>) -> Result<(), std::io::Error> {
    let n_games: i32 = wins.iter().sum();
    writer.write_fmt(format_args!("n_games:{n_games}\n"))?;
    for (pn, p_wins) in wins.iter().enumerate() {
        writer.write_fmt(format_args!("{pn}:{p_wins}\n"))?
    }
    let winner = wins
        .iter()
        .enumerate()
        .max_by_key(|(_, &value)| value)
        .map(|(idx, _)| idx)
        .unwrap(); // argmax

    writer.write_fmt(format_args!("winner:{winner}\n"))?;

    Ok(())
}

struct MatchStats {
    n_players: usize,
    timeouts: Vec<i32>,
    invalid_inputs: Vec<i32>,
    losing_moves: Vec<i32>,
    wins: Vec<i32>,
}

impl MatchStats {
    fn new(n_players: usize) -> Self {
        Self {
            n_players,
            timeouts: vec![0; n_players],
            invalid_inputs: vec![0; n_players],
            losing_moves: vec![0; n_players],
            wins: vec![0; n_players],
        }
    }

    fn update(&mut self, player_results: Vec<&PlayerResult>) {
        use LossReason as LR;
        use PlayerResult as PR;
        for (player, result) in player_results.iter().enumerate() {
            match result {
                PR::Loser(LR::InvalidInput) => self.invalid_inputs[player] += 1,
                PR::Loser(LR::LosingMove) => self.losing_moves[player] += 1,
                PR::Loser(LR::TimeOut) => self.timeouts[player] += 1,
                PR::Winner => self.wins[player] += 1,
            }
        }
    }

    fn most_wins(&self) -> Vec<usize> {
        let Some(max_wins) = self.wins.iter().max() else {
            return Vec::new();
        };
        (0..self.n_players)
            .filter(|player| self.wins[*player] == *max_wins)
            .collect()
    }
}

pub fn play_match(
    scripts: Vec<&str>,
    width: usize,
    height: usize,
    n_games: usize,
    time_limit: u64,
    summary_filename: &Path,
    gamelogs_folder: Option<PathBuf>,
) -> usize {
    if let Some(folder_name) = &gamelogs_folder {
        // create folder for logs if needed
        std::fs::create_dir_all(folder_name).unwrap();
    }
    let mut gamelog_path = gamelogs_folder; // rename because we will be adding filename
    gamelog_path.as_mut().map(|path| path.push("log.txt")); // dummy file name, will be replaced
    let n_players = scripts.len();

    let mut logwriter = LineWriter::new(File::create(summary_filename).unwrap());

    // keeping track of stats
    // let mut wins = vec![0; n_players];
    // let mut timeouts = vec![0; n_players];
    // let mut invalid_inputs = vec![0; n_players];
    // let mut losing_moves = vec![0; n_players];
    let mut match_stats = MatchStats::new(n_players);
    let mut tagged_scripts: Vec<(usize, &str)> = scripts.into_iter().enumerate().collect();

    let mut shuffled_players: Vec<usize>;
    let mut shuffled_scripts: Vec<&str>;

    for gameno in 0..n_games {
        tagged_scripts.shuffle(&mut thread_rng()); // shuffle player ids and scripts together so we can unshuffle the results from the game (i.e. the player corresponding to the i'th index after shuffling has id shuffled_players[i])
        (shuffled_players, shuffled_scripts) = tagged_scripts
            .iter()
            .map(|(n, script)| (n, *script))
            .unzip();

        gamelog_path
            .as_mut()
            .map(|path| path.set_file_name(format!("log{gameno}.txt")));

        if let Some((winner, player_results)) = play_game(
            &shuffled_scripts,
            None,
            width,
            height,
            gamelog_path.as_deref(),
            false,
            time_limit,
        ) {
            // wins[shuffled_players[winner]] += 1;
            let unshuffled_player_results: Vec<&PlayerResult> = (0..n_players)
                .map(|idx| &player_results[shuffled_players[idx]])
                .collect();

            match_stats.update(unshuffled_player_results);
            // println!("Game done {}", shuffled_players[winner])
        } else {
            // println!("Game failed, no winner")
        }
    }

    // Tie breaker: if two or more players share the highest amount of wins, we play another game, the winner of which is the winner of the match
    let max_wins = *match_stats.wins.iter().max().unwrap();
    // TODO: Error handling: make sure scripts.len != 0

    tagged_scripts = tagged_scripts
        .into_iter()
        .filter(|(pn, _)| match_stats.wins[*pn] == max_wins)
        .collect(); // filter only tied winning players

    if tagged_scripts.len() == 0 {
        panic!("") // TODO: handle error. Can only occur if scripts.len() == 0
    }
    if let [(pn, _)] = tagged_scripts[..] {
        log_summary(&mut logwriter, match_stats.wins).unwrap();
        return pn;
    } // length 1 => return

    // 2 or more tied players: tiebreaker
    let mut gameno = n_games - 1;
    for _ in 0..10 {
        // in case game fails, we try a few times
        // should only occur once,
        // but if play_game fails, might have to redo
        // 10 should be a safe margin
        tagged_scripts.shuffle(&mut thread_rng()); // shuffle player ids and scripts together so we can unshuffle the results from the game (i.e. the player corresponding to the i'th index after shuffling has id shuffled_players[i])
        (shuffled_players, shuffled_scripts) = tagged_scripts
            .iter()
            .map(|(n, script)| (n, *script))
            .unzip();

        gamelog_path
            .as_mut()
            .map(|path| path.set_file_name(format!("log{gameno}.txt")));

        if let Some((winner, player_results)) = play_game(
            &shuffled_scripts,
            None,
            width,
            height,
            gamelog_path.as_deref(),
            false,
            time_limit,
        ) {
            // wins[shuffled_players[winner]] += 1;
            let unshuffled_player_results: Vec<&PlayerResult> = (0..n_players)
                .map(|idx| &player_results[shuffled_players[idx]])
                .collect();

            match_stats.update(unshuffled_player_results);
            println!("Game done {}", shuffled_players[winner]);
            return match_stats.most_wins()[0];
        } else {
            println!("Game failed, no winner")
        }
        gameno += 1;
    }

    tagged_scripts[0].0 // failsafe
}

fn kill_player(
    player: usize,
    sender: &mpsc::Sender<Message>,
    reason: LossReason,
    player_statuses: &mut Vec<PlayerStatus>,
) {
    // alive_players.remove(&player);
    player_statuses[player] = PlayerStatus::Dead(reason);
    sender.send(Message::Kill(player)).unwrap();
}
