use shakmaty::{Chess, Color, Piece, Position, Role};

// ---------------------------------------------------------------- evaluation

// Order: pawn, knight, bishop, rook, queen, king
pub const VALUE: [i32; 6] = [100, 320, 330, 500, 900, 0];

// Tables are written from White's point of view, rank 8 first.
#[rustfmt::skip]
const PST: [[i32; 64]; 6] = [
    // pawn
    [  0,  0,  0,  0,  0,  0,  0,  0,
      50, 50, 50, 50, 50, 50, 50, 50,
      10, 10, 20, 30, 30, 20, 10, 10,
       5,  5, 10, 25, 25, 10,  5,  5,
       0,  0,  0, 20, 20,  0,  0,  0,
       5, -5,-10,  0,  0,-10, -5,  5,
       5, 10, 10,-20,-20, 10, 10,  5,
       0,  0,  0,  0,  0,  0,  0,  0],
    // knight
    [-50,-40,-30,-30,-30,-30,-40,-50,
     -40,-20,  0,  0,  0,  0,-20,-40,
     -30,  0, 10, 15, 15, 10,  0,-30,
     -30,  5, 15, 20, 20, 15,  5,-30,
     -30,  0, 15, 20, 20, 15,  0,-30,
     -30,  5, 10, 15, 15, 10,  5,-30,
     -40,-20,  0,  5,  5,  0,-20,-40,
     -50,-40,-30,-30,-30,-30,-40,-50],
    // bishop
    [-20,-10,-10,-10,-10,-10,-10,-20,
     -10,  0,  0,  0,  0,  0,  0,-10,
     -10,  0,  5, 10, 10,  5,  0,-10,
     -10,  5,  5, 10, 10,  5,  5,-10,
     -10,  0, 10, 10, 10, 10,  0,-10,
     -10, 10, 10, 10, 10, 10, 10,-10,
     -10,  5,  0,  0,  0,  0,  5,-10,
     -20,-10,-10,-10,-10,-10,-10,-20],
    // rook
    [  0,  0,  0,  0,  0,  0,  0,  0,
       5, 10, 10, 10, 10, 10, 10,  5,
      -5,  0,  0,  0,  0,  0,  0, -5,
      -5,  0,  0,  0,  0,  0,  0, -5,
      -5,  0,  0,  0,  0,  0,  0, -5,
      -5,  0,  0,  0,  0,  0,  0, -5,
      -5,  0,  0,  0,  0,  0,  0, -5,
       0,  0,  0,  5,  5,  0,  0,  0],
    // queen
    [-20,-10,-10, -5, -5,-10,-10,-20,
     -10,  0,  0,  0,  0,  0,  0,-10,
     -10,  0,  5,  5,  5,  5,  0,-10,
      -5,  0,  5,  5,  5,  5,  0, -5,
       0,  0,  5,  5,  5,  5,  0, -5,
     -10,  5,  5,  5,  5,  5,  0,-10,
     -10,  0,  5,  0,  0,  0,  0,-10,
     -20,-10,-10, -5, -5,-10,-10,-20],
    // king (middlegame)
    [-30,-40,-40,-50,-50,-40,-40,-30,
     -30,-40,-40,-50,-50,-40,-40,-30,
     -30,-40,-40,-50,-50,-40,-40,-30,
     -30,-40,-40,-50,-50,-40,-40,-30,
     -20,-30,-30,-40,-40,-30,-30,-20,
     -10,-20,-20,-20,-20,-20,-20,-10,
      20, 20,  0,  0,  0,  0, 20, 20,
      20, 30, 10,  0,  0, 10, 30, 20],
];

pub fn role_index(role: Role) -> usize {
    match role {
        Role::Pawn => 0,
        Role::Knight => 1,
        Role::Bishop => 2,
        Role::Rook => 3,
        Role::Queen => 4,
        Role::King => 5,
    }
}

fn passed_pawn_bonus(pos: &Chess) -> i32 {
    let board = pos.board();
    let mut score = 0;

    for color in Color::ALL {
        let enemy = match color {
            Color::White => Color::Black,
            Color::Black => Color::White,
        };

        for pawn in board.by_piece(Piece {
            color,
            role: Role::Pawn,
        }) {
            let square = pawn as usize;
            let file = square % 8;
            let rank = square / 8;

            let enemy_pawn_ahead = board
                .by_piece(Piece {
                    color: enemy,
                    role: Role::Pawn,
                })
                .into_iter().any(|other| {
                    let other = other as usize;
                    let other_file = other % 8;
                    let other_rank = other / 8;
                    let nearby_file = file.abs_diff(other_file) <= 1;

                    let ahead = match color {
                        Color::White => other_rank > rank,
                        Color::Black => other_rank < rank,
                    };

                    nearby_file && ahead
                });

            if !enemy_pawn_ahead {
                let advance = match color {
                    Color::White => rank.saturating_sub(1),
                    Color::Black => 6usize.saturating_sub(rank),
                };

                let bonus = match advance {
                    0 => 0,
                    1 => 10,
                    2 => 20,
                    3 => 35,
                    4 => 60,
                    5 => 100,
                    _ => 150,
                };

                score += match color {
                    Color::White => bonus,
                    Color::Black => -bonus,
                };
            }
        }
    }

    score
}

/// Score in centipawns from the point of view of the side to move.
pub fn evaluate(pos: &Chess) -> i32 {
    let board = pos.board();
    let mut score = 0; // from White's point of view
    for color in Color::ALL {
        for role in Role::ALL {
            let r = role_index(role);
            for sq in board.by_piece(Piece { color, role }) {
                let s = sq as usize; // a1 = 0 ... h8 = 63
                let (idx, sign) = match color {
                    Color::White => (s ^ 56, 1), // flip rank to match table layout
                    Color::Black => (s, -1),
                };
                score += sign * (VALUE[r] + PST[r][idx]);
            }
        }
    }

score += passed_pawn_bonus(pos);


    match pos.turn() {
        Color::White => score,
        Color::Black => -score,
    }
}

