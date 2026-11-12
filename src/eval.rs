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
score += king_safety(pos);
score += king_endgame_adjust(pos);
score += mop_up(pos);


    match pos.turn() {
        Color::White => score,
        Color::Black => -score,
    }
}


/// Pawn-shield bonus for each king, applied only while the enemy still has a queen.
fn king_safety(pos: &Chess) -> i32 {
    let board = pos.board();
    let mut score = 0;

    for color in Color::ALL {
        let enemy = match color {
            Color::White => Color::Black,
            Color::Black => Color::White,
        };
        // Only matters while the enemy still has a queen.
        if board
            .by_piece(Piece {
                color: enemy,
                role: Role::Queen,
            })
            .is_empty()
        {
            continue;
        }
        let Some(king) = board.king_of(color) else {
            continue;
        };
        let king_file = ((king as usize) % 8) as i32;
        let king_rank = ((king as usize) / 8) as i32;

        // Per file: 2 = pawn directly in front of the king, 1 = pawn two squares
        // in front, 0 = neither. `ahead` marks files with any pawn in front.
        let mut shield = [0i32; 8];
        let mut ahead = [false; 8];
        for pawn in board.by_piece(Piece {
            color,
            role: Role::Pawn,
        }) {
            let file = (pawn as usize) % 8;
            let rank = ((pawn as usize) / 8) as i32;
            let distance = match color {
                Color::White => rank - king_rank,
                Color::Black => king_rank - rank,
            };
            if distance >= 1 {
                ahead[file] = true;
                if distance == 1 {
                    shield[file] = 2;
                } else if distance == 2 && shield[file] == 0 {
                    shield[file] = 1;
                }
            }
        }

        let mut bonus = 0;
        for file in (king_file - 1).max(0)..=(king_file + 1).min(7) {
            let file = file as usize;
            bonus += match shield[file] {
                2 => 10,
                1 => 5,
                _ => -15,
            };
            if !ahead[file] {
                bonus -= 10;
            }
        }

        score += match color {
            Color::White => bonus,
            Color::Black => -bonus,
        };
    }
    score
}

const PHASE_MAX: i32 = 24;

// King table for endgames: reward a centralized king. Same layout as PST
// (White's point of view, rank 8 first).
#[rustfmt::skip]
const KING_ENDGAME: [i32; 64] = [
    -50,-40,-30,-20,-20,-30,-40,-50,
    -30,-20,-10,  0,  0,-10,-20,-30,
    -30,-10, 20, 30, 30, 20,-10,-30,
    -30,-10, 30, 40, 40, 30,-10,-30,
    -30,-10, 30, 40, 40, 30,-10,-30,
    -30,-10, 20, 30, 30, 20,-10,-30,
    -30,-30,  0,  0,  0,  0,-30,-30,
    -50,-30,-30,-30,-30,-30,-30,-50,
];

/// 24 with all pieces on the board, 0 with only kings and pawns.
fn game_phase(pos: &Chess) -> i32 {
    let board = pos.board();
    let mut phase = 0;
    for color in Color::ALL {
        for (role, weight) in [
            (Role::Knight, 1),
            (Role::Bishop, 1),
            (Role::Rook, 2),
            (Role::Queen, 4),
        ] {
            phase += weight * board.by_piece(Piece { color, role }).count() as i32;
        }
    }
    phase.min(PHASE_MAX)
}

/// The king table in `PST` is the middlegame one. As pieces come off the board,
/// blend toward a table that rewards a centralized king.
fn king_endgame_adjust(pos: &Chess) -> i32 {
    let board = pos.board();
    let phase = game_phase(pos);
    let mut score = 0;
    for color in Color::ALL {
        let Some(king) = board.king_of(color) else {
            continue;
        };
        let s = king as usize;
        let idx = match color {
            Color::White => s ^ 56,
            Color::Black => s,
        };
        let delta = (KING_ENDGAME[idx] - PST[5][idx]) * (PHASE_MAX - phase) / PHASE_MAX;
        score += match color {
            Color::White => delta,
            Color::Black => -delta,
        };
    }
    score
}

/// When one side is far ahead in an endgame, reward driving the enemy king to
/// the edge and bringing the own king close, so the engine makes progress
/// toward mate instead of shuffling.
fn mop_up(pos: &Chess) -> i32 {
    if game_phase(pos) > 12 {
        return 0;
    }
    let board = pos.board();
    let material = |color: Color| -> i32 {
        Role::ALL
            .iter()
            .map(|&role| VALUE[role_index(role)] * board.by_piece(Piece { color, role }).count() as i32)
            .sum()
    };
    let white = material(Color::White);
    let black = material(Color::Black);
    let (strong, weak, diff) = if white >= black {
        (Color::White, Color::Black, white - black)
    } else {
        (Color::Black, Color::White, black - white)
    };
    if diff < 400 {
        return 0;
    }
    let (Some(strong_king), Some(weak_king)) = (board.king_of(strong), board.king_of(weak)) else {
        return 0;
    };

    let sf = ((strong_king as usize) % 8) as i32;
    let sr = ((strong_king as usize) / 8) as i32;
    let wf = ((weak_king as usize) % 8) as i32;
    let wr = ((weak_king as usize) / 8) as i32;

    let edge = (3 - wf).max(wf - 4) + (3 - wr).max(wr - 4); // 0 in the centre, 6 in a corner
    let apart = (sf - wf).abs() + (sr - wr).abs(); // distance between the kings
    let bonus = 20 * edge + 8 * (14 - apart);

    match strong {
        Color::White => bonus,
        Color::Black => -bonus,
    }
}
