# snakerunner

##IO interface

###Header format
`{width},{height}`: dimensions of the board
`{n_players}`: amount of players in the game
`{x},{y}` (repeats `n_player` times): starting positions of the players
`{player_id}`: your player id in the range [`0`, `n_players`)

###Game inputs
`move`: respond with move (`N`, `S`,`E` or `W`)
`stop`: stop your program
`{player}:{direction}`: (e.g. `0:N`) indicates move made by player
`out:{player}`: player is out of the game
