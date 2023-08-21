import random

class TorusSnakeGame:
    direction_to_coord_shift = {"N": (0,-1), "S": (0,1), "E": (1,0), "W": (-1,0)}
    def __init__(self,width, height, starting_positions):
        self.board = [[-1 for i in range(width)] for j in range(height)]  # -1 is empty, otherwise contains player number
        self.width = width
        self.height = height
        self.head_positions = starting_positions  # positions of snake heads
        self.n_players = len(starting_positions)
        self.alive_players = set(range(self.n_players))
        for player, pos in enumerate(starting_positions):
            self.set_cell(*pos, player)

    def get_cell(self, x, y):
        return self.board[y][x]
    
    def set_cell(self, x, y, player):
        self.board[y][x] = player
    
    def shift_coords(self, x, y, direction): 
        """Returns coordinates of position (x,y) shifted one step in the given direction"""
        dx, dy = self.direction_to_coord_shift[direction]
        new_x = (x + dx) % self.width
        new_y = (y + dy) % self.height
        return new_x, new_y

    def move_player(self, player, direction):
        """Moves player in given direction if possible and returns True. If instead the cell is occupied,
        does nothing and returns False"""
        new_pos = self.shift_coords(*self.head_positions[player], direction)
        if self.get_cell(*new_pos) == -1:
            self.head_positions[player] = new_pos
            self.set_cell(*new_pos, player)
            return True
        return False
    
    def display_cell(self, x, y):
        if self.get_cell(x,y) == -1:
            return "·"

        player = self.get_cell(x,y)
        if self.head_positions[player] == (x,y):
            if player in self.alive_players:
               return "\033[1m\033[32m" + str(player) + "\033[0m" 
            else:
                return "\033[1m\033[31m" + str(player) + "\033[0m" 
        else:
            return str(player)
    
    def __str__(self):
        return "\n".join(["".join([self.display_cell(x,y) for x in range(self.width)]) for y in range(self.height)])
    


width, height = [int(s) for s in input().split(",")] # width and height of board
n_players = int(input()) # number of players
starting_positions = []
for i in range(n_players):
    starting_positions.append(tuple(int(s) for s in input().split(",")))
my_player_number = int(input())


game = TorusSnakeGame(width, height, starting_positions)
directions = ["N", "S", "E", "W"]
while True:
    instruction = input()
    if instruction == "stop":
        break
    elif instruction == "move":
        random.shuffle(directions)
        for direction in directions:
            if game.move_player(my_player_number, direction): # check if move doesn't lose (executes move too)
                print(direction)
                break
        else: # if we never break, all directions lose, so just do any direction
            print("N")
    elif instruction[:3] == "out":
        dead_player = int(instruction[4:])
        game.alive_players.remove(dead_player)
    else: # instruction must be a move from another player
        player_number, direction = instruction.split(":")
        game.move_player(int(player_number), direction)
    