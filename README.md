# snakerunner
A command line tool for running a competitive game of snake with looping edges between scripts through an I/O interface with the `run` command. The `show` command plays a stored in the terminal.

## Command line tool
- `snakerunner run -s <SCRIPTS>`. Runs a game of Snake on a Torus between the scripts. See `snakerunner run -h` for more details and settings. Scripts ending in `.py` will be run as python scripts, anything else will be assumed to be an executable.
- `snakerunner show`. Plays a stored game from a log file in the terminal. See `snakerunner show -h` for more details and settings.
- `snakerunner match -s <SCRIPTS> -n <N_GAMES>`. Runs `N-GAMES` games with starting positions and move order being randomized each time. Plays a tiebreaker if necessary.  See `snakerunner match -h` for more details and settings.


## Examples
### Run and view a game
Run `snakerunner run -s unidirectionalmover.py randommover.py`. This runs a game between the two scripts and logs the moves to `log.txt`. The winner will be shown in the terminal.

Now run `snakerunner show`. This loads `log.txt` and replays the game visually in the terminal.

### Run a match
Run `snakerunner match -s unidirectionalmover.py randommover.py -n 100`. The winner will be shown in the terminal. Look in `summary.txt` to see how many games each player won.

## Rules of Snake on a Torus
The game is played on a grid, however, moving over an edge of the grid makes the head of the snake appear on the opposite side. Unlike the classic game of snake, the snake does not stay a fixed length, but rather keeps growing, leaving its tail in place. If you move onto another snake, you die. Dead snakes remain in the playing field, and hitting them is still fatal. Your goal is to stay alive the longest, by trapping your opponents and avoiding getting trapped yourself. The last remaining player wins. 

## IO interface
If you want to write your own script to play snake, it needs to communicate with the following interface. All interaction goes through `stdin` and `stdout`. First, a header is sent to your program, indicating the setup of the game. Then, inputs to your program tell it what moves other players have made and whether action is required from your script. Have a look at `randommover.py` and `unidirectionalmover.py` for an example implementation of the game and handling I/O.

### Header format
`{width},{height}`: dimensions of the board

`{n_players}`: amount of players in the game

`{x},{y}` (repeats `n_player` times): starting positions of the players

`{player_id}`: your player id in the range [`0`, `n_players`)
#### Example 
```
10,10
3
0,2
1,4
9,6
2
```

### Game inputs
- `move`: instruction to respond with a move (`N`, `S`,`E` or `W`). Make sure your response ends with a newline. [Currently, timeout is 100ms]
- `stop`: instruction stop your program. 
- `{player}:{direction}`: (e.g. `0:N`) indicates move made by player. Your own moves are not sent back to you.
- `out:{player}`: player is out of the game.
  
Note that `stop` requires you to quit your script, while `out:{player}` indicates that another script has stopped. The latter requires no action from you, it just informs you that that particular snake will not move anymore. In both cases, the reason that the program has stopped could be anything, such as simply losing the game, the script crashing, timeout, invalid input, etc. 

## Included
The `snakerunner` and `snakerunner.exe` files are compiled programs for unix and windows respectively. In your script, you will need to include your own model of the game. `randommover.py` has the class `TorusSnakeGame` as an example implementation (`unidirectionalmover.py` has the same one). For a rust example script, see `unidirectionalmover.rs`. The examples also include code for parsing the header and game instructions, which are guaranteed to work*.

*probably
