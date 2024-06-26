use bevy::{prelude::*, utils::hashbrown::HashMap};
use bevy_ggrs::{
    ggrs::{InputStatus, PlayerType, SessionBuilder},
    GgrsConfig, GgrsPlugin, GgrsSchedule, LocalInputs, LocalPlayers, PlayerInputs, ReadInputs,
    Session,
};
use bevy_matchbox::{
    matchbox_socket::{PeerId, PeerState, SingleChannel},
    MatchboxSocket,
};
use bytemuck::{Pod, Zeroable};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            GgrsPlugin::<MyGgrsConfig>::default(),
            MainPlugin,
        ))
        .run();
}

const MATCHBOX_ADDR: &str = "ws://localhost:3536";
// const MATCHBOX_ADDR: &str = "ws://match-0-7.helsing.studio/";
const ROOM_ID: &str = "peq";
struct MainPlugin;

impl Plugin for MainPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_systems(Startup, start_matchbox_socket)
            .add_systems(ReadInputs, read_local_inputs)
            .add_systems(
                Update,
                (
                    update_matchbox_socket.run_if(in_state(AppState::MenuConnect)),
                    log_ggrs_events.run_if(in_state(AppState::RoundOnline)),
                ),
            )
            .add_systems(GgrsSchedule, apply_inputs);
    }
}

#[repr(C)]
#[derive(Debug, Pod, Copy, Clone, Zeroable, PartialEq, Resource)]
struct MyInput(u8);

fn read_local_inputs(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    local_players: Res<LocalPlayers>,
) {
    let mut local_inputs = HashMap::new();

    for handle in &local_players.0 {
        let mut input = 0;
        if keyboard_input.just_pressed(KeyCode::Space) {
            info!("{handle}");
            input = 1;
        }

        local_inputs.insert(*handle, MyInput(input));
    }

    commands.insert_resource(LocalInputs::<MyGgrsConfig>(local_inputs));
}

fn start_matchbox_socket(mut commands: Commands) {
    let room_url = format!("{MATCHBOX_ADDR}/{ROOM_ID}");
    let socket = MatchboxSocket::new_ggrs(room_url);
    commands.insert_resource(socket);
}

fn apply_inputs(inputs: Res<PlayerInputs<MyGgrsConfig>>) {
    for i in 0..inputs.len() {
        let input = match inputs[i].1 {
            InputStatus::Confirmed => inputs[i].0,
            InputStatus::Predicted => inputs[i].0,
            InputStatus::Disconnected => MyInput(0), // disconnected players do nothing
        };

        if input.0 != 0 {
            info!("input {}", input.0);
        }
    }
}

#[derive(States, Debug, Clone, Eq, PartialEq, Hash, Default)]
enum AppState {
    #[default]
    MenuConnect,
    RoundOnline,
}

const NUM_PLAYERS: usize = 2;
const FPS: usize = 60;
const MAX_PREDICTION: usize = 12;
const INPUT_DELAY: usize = 2;
// const CHECK_DISTANCE: usize = 2;

type MyGgrsConfig = GgrsConfig<MyInput, PeerId>;

fn update_matchbox_socket(
    mut commands: Commands,
    mut state: ResMut<NextState<AppState>>,
    mut socket: ResMut<MatchboxSocket<SingleChannel>>,
) {
    // regularly call update_peers to update the list of connected peers
    for (peer, new_state) in socket.update_peers() {
        // you can also handle the specific dis(connections) as they occur:
        match new_state {
            PeerState::Connected => info!("peer {peer} connected"),
            PeerState::Disconnected => info!("peer {peer} disconnected"),
        }
    }

    if socket.players().len() >= NUM_PLAYERS {
        // create a new ggrs session
        let mut sess_build = SessionBuilder::<MyGgrsConfig>::new()
            .with_num_players(NUM_PLAYERS)
            .with_max_prediction_window(MAX_PREDICTION)
            .expect("Invalid prediction window")
            .with_fps(FPS)
            .expect("Invalid FPS")
            .with_input_delay(INPUT_DELAY);

        // add players
        let mut handles = Vec::new();
        for (i, player_type) in socket.players().iter().enumerate() {
            if *player_type == PlayerType::Local {
                handles.push(i);
            }
            sess_build = sess_build
                .add_player(player_type.clone(), i)
                .expect("Invalid player added.");
        }

        // start the GGRS session
        let channel = socket.take_channel(0).unwrap();
        let sess = sess_build
            .start_p2p_session(channel)
            .expect("Session could not be created.");

        // insert session as resource and switch state
        commands.insert_resource(Session::P2P(sess));
        // commands.insert_resource(LocalPlayers(handles));
        state.set(AppState::RoundOnline);
    }
}

fn log_ggrs_events(mut session: ResMut<Session<MyGgrsConfig>>) {
    match session.as_mut() {
        Session::P2P(s) => {
            for event in s.events() {
                info!("GGRS Event: {event:?}");
            }
        }
        _ => panic!("This example focuses on p2p."),
    }
}
