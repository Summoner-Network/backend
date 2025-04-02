use std::collections::HashMap;

use library::*;
use macros::entrypoint;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
struct Root {
    session_record: Table<address /* player */, u64 /* number of sessions for this player */>,
    sessions: Table<(
            address /* player */,
            u64 /* session id for player */
        ),
        Pointer<RPS
    > /* stored under both players at respective positions */>
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
enum Move {
    ROCK,
    PAPER,
    SCISSORS,
    INVALID
}

#[derive(Serialize, Deserialize, Clone)]
struct RPS {
    players: (address, address),
    history: (Vec<Move>, Vec<Move>),
    scoring: (u32, u32),
    ante_in: u64,
    commits: (Option<bytea>, Option<bytea>),
    coomits_reveal: Option<((Move, bytea /* salt */), (Move, bytea /* salt */))>
}

impl RPS {
    /// Initializes a new RPS session with an ante amount and persists it.
    pub fn new(player1: address, player2: address, ante_in: u64) -> Self {
        let mut root = get_root::<Root>().unwrap_or(Root {
            session_record: Table::new("session_record".to_string()),
            sessions: Table::new("sessions".to_string()),
        });

        let player1_sessions = root.session_record.get(&player1).get().unwrap_or(0);
        let player2_sessions = root.session_record.get(&player2).get().unwrap_or(0);

        let game_id = player1_sessions.max(player2_sessions) + 1;

        root.session_record.set(&player1, player1_sessions + 1);
        root.session_record.set(&player2, player2_sessions + 1);

        let rps = Self {
            players: (player1.clone(), player2.clone()),
            history: (Vec::new(), Vec::new()),
            scoring: (0, 0),
            ante_in,
            commits: (None, None),
            coomits_reveal: None,
        };

        let pointer = Pointer::new(None);
        pointer.set(rps.clone());

        root.sessions.set(&(player1.clone(), game_id), pointer.clone());
        root.sessions.set(&(player2.clone(), game_id), pointer.clone());

        set_root(root);

        rps
    }

    /// Commits a player's move using a hash (commit phase).
    pub fn commit_move(player: address, game_id: u64, commit: Vec<u8>) {
        let mut root = get_root::<Root>().unwrap();
        let pointer = root.sessions.get(&(player.clone(), game_id)).get().unwrap();
        let mut session = pointer.get().unwrap();

        if session.players.0 == player {
            if session.commits.0 == None {
                session.commits.0 = Some(commit);
            }
        } else if session.players.1 == player {
            if session.commits.0 == None {
                session.commits.1 = Some(commit);
            }
        }

        pointer.set(session);
    }

    /// Reveals a player's move and verifies it against their commit.
    pub fn reveal_move(player: address, game_id: u64, reveal: Move, salt: bytea) -> bool {
        let mut root = get_root::<Root>().unwrap();
        let pointer = root.sessions.get(&(player.clone(), game_id)).get().unwrap();
        let mut session = pointer.get().unwrap();

        let commit = if session.players.0 == player {
            session.commits.0.clone()
        } else {
            session.commits.1.clone()
        };

        if let Some(commit) = commit {
            if verify_commit_reveal(commit, (&reveal, &salt)) {
                if session.players.0 == player {
                    session.history.0.push(reveal);
                } else if session.players.1 == player {
                    session.history.1.push(reveal);
                }

                pointer.set(session);
                return true;
            }
        }
        false
    }

    /// Determines the winner of a round and updates scores accordingly.
    pub fn determine_round(game_id: u64, player: address) -> Option<address> {
        let root = get_root::<Root>().unwrap();
        let pointer = root.sessions.get(&(player.clone(), game_id)).get()?;
        let mut session = pointer.get()?;

        let move1 = session.history.0.last()?;
        let move2 = session.history.1.last()?;

        let winner = match (move1, move2) {
            (Move::ROCK, Move::SCISSORS)
            | (Move::SCISSORS, Move::PAPER)
            | (Move::PAPER, Move::ROCK) => Some(session.players.0.clone()),

            (Move::SCISSORS, Move::ROCK)
            | (Move::PAPER, Move::SCISSORS)
            | (Move::ROCK, Move::PAPER) => Some(session.players.1.clone()),

            _ => None, // It's a tie
        };

        session.update_scores(winner.clone());
        pointer.set(session);

        winner
    }

    /// Updates the game state after each round.
    pub fn update_scores(&mut self, winner: Option<address>) {
        if let Some(winner_addr) = winner {
            if winner_addr == self.players.0 {
                self.scoring.0 += 1;
            } else if winner_addr == self.players.1 {
                self.scoring.1 += 1;
            }
        }
    }
}

#[entrypoint]
pub fn contract(input: Input) -> Output {
    let mut results: HashMap<String, Vec<Option<Value>>> = HashMap::new();

    for (func, calls) in input.functions.into_iter() {
        let mut func_results = Vec::new();

        for call in calls {
            let args: Vec<Value> = if call.is_array() {
                call.as_array().unwrap().clone()
            } else {
                vec![call]
            };

            let res = match func.as_str() {
                "new" => {
                    if args.len() != 3 {
                        None
                    } else {
                        let player1 = args[0].as_str();
                        let player2 = args[1].as_str();
                        let ante_in = args[2].as_u64();
                        if let (Some(player1), Some(player2), Some(ante_in)) = (player1, player2, ante_in) {
                            let session = RPS::new(player1.into(), player2.into(), ante_in);
                            serde_json::to_value(session).ok()
                        } else {
                            None
                        }
                    }
                }
                "commit_move" => {
                    if args.len() != 3 {
                        None
                    } else {
                        let player = args[0].as_str();
                        let game_id = args[1].as_u64();
                        let commit = args[2].as_str().and_then(|s| hex::decode(s).ok());
                        if let (Some(player), Some(game_id), Some(commit)) = (player, game_id, commit) {
                            RPS::commit_move(player.into(), game_id, commit);
                            Some(Value::Bool(true))
                        } else {
                            None
                        }
                    }
                }
                "reveal_move" => {
                    if args.len() != 4 {
                        None
                    } else {
                        let player = args[0].as_str();
                        let game_id = args[1].as_u64();
                        let move_str = args[2].as_str();
                        let salt = args[3].as_str().and_then(|s| hex::decode(s).ok());
                        if let (Some(player), Some(game_id), Some(move_str), Some(salt)) = (player, game_id, move_str, salt) {
                            let move_enum = match move_str {
                                "ROCK" => Move::ROCK,
                                "PAPER" => Move::PAPER,
                                "SCISSORS" => Move::SCISSORS,
                                _ => Move::INVALID,
                            };
                            if move_enum == Move::INVALID {
                                None
                            } else {
                                let result = RPS::reveal_move(player.into(), game_id, move_enum, salt);
                                Some(Value::Bool(result))
                            }
                        } else {
                            None
                        }
                    }
                }
                "determine_round" => {
                    if args.len() != 2 {
                        None
                    } else {
                        let game_id = args[0].as_u64();
                        let player = args[1].as_str();
                        if let (Some(game_id), Some(player)) = (game_id, player) {
                            let winner = RPS::determine_round(game_id, player.into());
                            Some(serde_json::json!(winner))
                        } else {
                            None
                        }
                    }
                }
                _ => None,
            };

            func_results.push(res);
        }

        results.insert(func, func_results);
    }

    Output { functions: results }
}