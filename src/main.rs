use std::{ f32::{ consts::TAU,}, path::{ Path, PathBuf }, vec };

use bevy::{ prelude::*, sprite::Anchor };
use rand::Rng;

const WINDOW_WIDTH: f32 = 1920.;
const WINDOW_HEIGHT: f32 = 1080.0;

const VISION_DISTANCE: f32 = 80.0;
const VISION_ANGLE: f32 = TAU - TAU / 4.0;

const SEPARATION_INTENSITY: f32 = 1.0;
const ALIGNMENT_INTENSITY: f32 = 1.0;
const COHESION_INTENSITY: f32 = 1.0;
const AVOIDANCE_INTENSITY: f32 = 1.0;
//Ideas for other sliders:
// Aversion: how much boids avoid boids of other species, may be negative to induce "attraction"



const MAX_BOID_SPEED: f32 = 5.0;
const MIN_BOID_SPEED: f32 = 0.5;

fn main() {
    //create bevy app
    App::new()
        .add_plugins(
            DefaultPlugins.set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Boids".to_string(),
                        resolution: (WINDOW_WIDTH, WINDOW_HEIGHT).into(),
                        resizable: false,
                        decorations: true,
                        visible: true,
                        ..default()
                    }),
                    ..default()
                })
                .build()
        )

        .add_systems(Startup, (init, define_species, spawn_boids).chain())
        .add_systems(FixedUpdate, (update_boids, spawn_tails, handle_tails))
        .run();
}

pub fn init(mut commands: Commands) {
    commands.spawn(Camera2d::default());
}

pub fn spawn_tails(mut commands: Commands, boids: Query<(&Boid, &Transform)>) {
    for (b, tr) in boids.iter() {
        b.spawn_tail(&mut commands, tr.clone());
    }
}

pub fn handle_tails(
    mut commands: Commands,
    time: Res<Time>,
    mut tails: Query<(Entity, &mut Tail)>
) {
    tails.par_iter_mut().for_each(|(e, mut t)| {
        //reduce the tail's alpha
        let alpha = t.color.alpha() - 0.1 * time.delta_secs();
        t.color.set_alpha(alpha);
        if t.color.alpha() < 0.2 {
            t.color.set_alpha(0.2);
        }

        //and reduce the tail's lifetime
        t.lifetime -= time.delta_secs();
    });

    for (e, t) in tails.iter_mut() {
        if t.lifetime <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

pub fn spawn_boids(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    species: Query<&Species>
) {
    let mut random = rand::rng();
    for spec in species.iter() {
        //spawn a herd of this species
        for _ in 0..200 {
            let img = asset_server.load(spec.img_path.clone());
            let x: f32 = random.random_range(-WINDOW_WIDTH / 2.0..WINDOW_WIDTH / 2.0);
            let y: f32 = random.random_range(-WINDOW_HEIGHT / 2.0..WINDOW_HEIGHT / 2.0);

            let vx: f32 = random.random_range(spec.speed_range.x..spec.speed_range.y);
            let vy: f32 = random.random_range(spec.speed_range.x..spec.speed_range.y);
            commands.spawn((
                Boid {
                    velocity: Vec2::new(vx, vy),
                    facing_angle: 0.0,
                    species: Some(spec.clone()),
                    tail_color: spec.tail_color,
                },
                Sprite {
                    image: img,
                    custom_size: Some(Vec2::new(10.0, 10.0) * spec.scale),
                    anchor: Anchor::Center,
                    ..default()
                },
                Transform {
                    translation: Vec3::new(x, y, 0.0),
                    rotation: Quat::from_rotation_z(0.0),
                    ..Default::default()
                },
            ));
        }
    }
}

pub fn define_species(mut commands: Commands) {
    let species1 = Species {
        img_path: PathBuf::from("textures\\boid.png"),
        intensities: Vec4::new(2.0, 1.0, 1.0, 2.0),
        vision_distance: 80.0,
        vision_angle: TAU - TAU / 2.0,
        speed_range: Vec2::new(0.1, 0.2),
        tail_color: Color::srgba(0.0, 0.0, 1.0, 1.0),
        scale: 3.
    };
    commands.spawn(species1);

    let species2 = Species {
        img_path: PathBuf::from("textures\\boid2.png"),
        intensities: Vec4::new(1.0, 1.0, 1.0, 1.0),
        vision_distance: 60.0,
        vision_angle: TAU - TAU / 3.0,
        speed_range: Vec2::new(4., 10.),
        tail_color: Color::srgba(1.0, 0.0, 0.0, 1.0),
        scale: 4.
    };
    commands.spawn(species2);
}

pub fn update_boids(commands: Commands, mut boids: Query<(&mut Boid, &mut Transform)>) {
    //clone boids' iterator so we can iterate over them twice
    let mut boid_collection = vec![];

    for (b, tr) in boids.iter_mut() {
        boid_collection.push((b.clone(), tr.clone()));
    }

    boids.par_iter_mut().for_each(|(mut b1, mut tr1)| {
        //collect all boids that are within the vision of the current boid
        let mut visible_boids: Vec<(&Boid, &Transform)> = Vec::new();
        for (b2, tr2) in boid_collection.iter() {
            if tr1.translation != tr2.translation && tr1.rotation != tr2.rotation {
                //make sure we're not comparing the same boid, the combination of translation and rotation should almost most definitley be unique
                if tr1.translation.distance(tr2.translation) < VISION_DISTANCE {
                    //check if boid is within vision distance
                    //the VISION_ANGLE describes a vision cone, check that the angle between the two boids is within the vision cone (cone rotates relative to boid.facing_angle)
                    let angle = (tr2.translation - tr1.translation).angle_between(
                        Vec3::new(b1.facing_angle.cos(), b1.facing_angle.sin(), 0.0)
                    );
                    if angle < VISION_ANGLE / 2.0 {
                        visible_boids.push((b2, tr2));
                    }
                }
            }
        }

        let mut separation = Vec2::ZERO;
        let mut alignment = Vec2::ZERO;
        let mut cohesion = Vec2::ZERO;

        for (b2, tr2) in visible_boids.iter() {
            let distance = tr1.translation.distance(tr2.translation);
            let direction = tr2.translation - tr1.translation;
            let direction = direction.normalize();
            
            //separation
            separation -= Vec2::new(direction.x, direction.y) / (distance / VISION_DISTANCE);
            
            //check if boid is the same species as the current boid, if not continue, boids only care to align and cohese with their own species just want to avoid running into anything else
            if b1.species != b2.species {
                continue;
            }

            //alignment
            alignment += b2.velocity;

            //cohesion
            cohesion += Vec2::new(direction.x, direction.y);
        }
        //avoidance
        //avoidance is a special case of separation, we will adjust velocity based on the inverse square of the normalized distance to the wall
        let mut avoidance = Vec2::ZERO;
        if tr1.translation.x < -WINDOW_WIDTH / 2.0 + VISION_DISTANCE {
            let distance_to_wall = (tr1.translation.x + WINDOW_WIDTH / 2.0) / VISION_DISTANCE;
            avoidance += Vec2::new(1.0, 0.0) / distance_to_wall.powf(2.0);
        } else if tr1.translation.x > WINDOW_WIDTH / 2.0 - VISION_DISTANCE {
            let distance_to_wall = (WINDOW_WIDTH / 2.0 - tr1.translation.x) / VISION_DISTANCE;
            avoidance -= Vec2::new(1.0, 0.0) / distance_to_wall.powf(2.0);
        }
        if tr1.translation.y < -WINDOW_HEIGHT / 2.0 + VISION_DISTANCE {
            let distance_to_wall = (tr1.translation.y + WINDOW_HEIGHT / 2.0) / VISION_DISTANCE;
            avoidance += Vec2::new(0.0, 1.0) / distance_to_wall.powf(2.0);
        } else if tr1.translation.y > WINDOW_HEIGHT / 2.0 - VISION_DISTANCE {
            let distance_to_wall = (WINDOW_HEIGHT / 2.0 - tr1.translation.y) / VISION_DISTANCE;
            avoidance -= Vec2::new(0.0, 1.0) / distance_to_wall.powf(2.0);
        }

        //apply separation, alignment, cohesion, and avoidance intensities
        if let Some(spec) = &b1.species {
            separation *= spec.intensities.x;
            alignment *= spec.intensities.y;
            cohesion *= spec.intensities.z;
            avoidance *= spec.intensities.w;
        } else {
            separation *= SEPARATION_INTENSITY;
            alignment *= ALIGNMENT_INTENSITY;
            cohesion *= COHESION_INTENSITY;
            avoidance *= AVOIDANCE_INTENSITY;
        }

        b1.velocity += separation;
        b1.velocity += alignment;
        b1.velocity += cohesion;
        b1.velocity += avoidance;
        //we will normalize the velocity later so we don't need to worry about the intensities being too high or low, really just the ratios matter

        //rotate boid to face it's velocity
        b1.facing_angle = b1.velocity.y.atan2(b1.velocity.x);
        tr1.rotation = Quat::from_rotation_z(b1.facing_angle);

        //clamp veloicty between min and max speed
        let speed = b1.velocity.length();
        if speed > MAX_BOID_SPEED {
            b1.velocity = b1.velocity.normalize() * MAX_BOID_SPEED;
        } else if speed < MIN_BOID_SPEED {
            b1.velocity = b1.velocity.normalize() * MIN_BOID_SPEED;
        }

        //move boid by its velocity
        tr1.translation += b1.velocity.extend(0.0);

        //if boid goes off screen, wrap around and then some, screen centered at 0,0,
        //use some padding so we don't clip back every frame,
        //boids velocity is not affected by wrapping so they can still move off screen a bit but wont be clipped back immediately because they are placed exactly on the screen again
        let padding = 5.0;
        if tr1.translation.x > WINDOW_WIDTH / 2.0 + padding {
            tr1.translation.x = -WINDOW_WIDTH / 2.0;
        } else if tr1.translation.x < -WINDOW_WIDTH / 2.0 - padding {
            tr1.translation.x = WINDOW_WIDTH / 2.0;
        }
        if tr1.translation.y > WINDOW_HEIGHT / 2.0 + padding {
            tr1.translation.y = -WINDOW_HEIGHT / 2.0;
        } else if tr1.translation.y < -WINDOW_HEIGHT / 2.0 - padding {
            tr1.translation.y = WINDOW_HEIGHT / 2.0;
        }
    });
}

#[derive(Component, Debug, Clone)]
pub struct Tail {
    point: Vec2,
    color: Color,
    lifetime: f32,
}

impl Default for Tail {
    fn default() -> Self {
        Tail {
            point: Vec2::ZERO,
            color: Color::WHITE,
            lifetime: 5.0,
        }
    }
}
impl Tail {
    pub fn new(point: Vec2, color: Color, lifetime: f32) -> Self {
        Tail {
            point,
            color,
            lifetime,
        }
    }
}

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Species {
    img_path: PathBuf,
    intensities: Vec4, //se, al, co, av
    vision_distance: f32,
    vision_angle: f32,
    speed_range: Vec2, //min, max
    tail_color: Color,
    scale: f32,
}

#[derive(Component, Debug, Clone)]
pub struct Boid {
    velocity: Vec2,
    facing_angle: f32,
    species: Option<Species>,
    tail_color: Color,
}

impl Boid {
    pub fn spawn_tail(&self, commands: &mut Commands, tr: Transform) {
        //spawn a tail
        commands.spawn((
            Tail::new(Vec2::new(tr.translation.x, tr.translation.y), self.tail_color, 1.0),
            Sprite {
                custom_size: Some(Vec2::new(1.0, 1.0)),
                color: self.tail_color,
                ..default()
            },
            Transform {
                translation: Vec3::new(tr.translation.x, tr.translation.y, 0.0),
                ..Default::default()
            },
        ));
    }
}
impl Default for Boid {
    fn default() -> Self {
        let mut random = rand::rng();
        let x: f32 = random.random_range(-MAX_BOID_SPEED..MAX_BOID_SPEED);
        let y: f32 = random.random_range(-MAX_BOID_SPEED..MAX_BOID_SPEED);

        let theta = random.random_range(0.0..TAU);

        Boid {
            velocity: Vec2::new(x, y),
            facing_angle: theta,
            species: None,
            tail_color: Color::WHITE,
        }
    }
}
