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
const ROOM_ID: &str = "peq";
struct MainPlugin;

impl Plugin for MainPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .add_systems(Startup, start_matchbox_socket)
            .add_systems(ReadInputs, input_handler)
            .add_systems(
                Update,
                (update_matchbox_socket.run_if(in_state(AppState::MenuConnect)),),
            )
            .add_systems(GgrsSchedule, apply_inputs);
    }
}

#[repr(C)]
#[derive(Debug, Pod, Copy, Clone, Zeroable, PartialEq, Resource)]
struct MyInput;

fn input_handler(mut commands: Commands, keyboard_input: Res<ButtonInput<KeyCode>>) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        commands.insert_resource(LocalInputs::<MyGgrsConfig>(HashMap::from([(
            0 as usize,
            MyInput {},
        )])));
    }
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
            InputStatus::Disconnected => MyInput, // disconnected players do nothing
        };

        info!("input");
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
        commands.insert_resource(LocalPlayers(handles));
        state.set(AppState::RoundOnline);
    }
}
