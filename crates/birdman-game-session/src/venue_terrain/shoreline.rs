use super::*;

#[derive(Clone, Copy)]
struct Transition {
    direction: [f64; 2],
    width: f64,
    height: f64,
}

struct Context {
    ordered: Vec<(usize, Grid)>,
    patches: Vec<Bounds>,
    maximum: f64,
    increment: f64,
    minimum_step: f64,
}

impl Context {
    fn mapped(&self, north: f64, east: f64) -> bool {
        for (_, grid) in &self.ordered {
            let column =
                ((east - grid.east_min_meters) / grid.terrain_step_meters + 0.5).floor() as isize;
            let row = (grid.rows as f64
                - 1.0
                - (north - grid.north_min_meters) / grid.terrain_step_meters
                + 0.5)
                .floor() as isize;
            if column >= 0 && column < grid.columns as isize && row >= 0 && row < grid.rows as isize
            {
                return grid.land_mask[row as usize * grid.columns + column as usize];
            }
        }
        true
    }

    fn surface(&self, north: f64, east: f64) -> Option<f64> {
        self.ordered.iter().find_map(|(index, grid)| {
            if *index == 0
                && self
                    .patches
                    .iter()
                    .any(|bounds| bounds.interior(north, east))
            {
                None
            } else {
                grid.height_at(north, east)
            }
        })
    }

    fn find_land(&self, point: [f64; 2], direction: [f64; 2]) -> Option<Transition> {
        let mut width = 8.0;
        loop {
            let north = point[0] + direction[0] * width;
            let east = point[1] + direction[1] * width;
            if self.mapped(north, east)
                && let Some(height) = self.surface(north, east)
            {
                return Some(Transition {
                    direction,
                    width,
                    height,
                });
            }
            if width >= self.maximum {
                return None;
            }
            width += self.increment;
        }
    }

    fn nearby(&self, point: [f64; 2], ring: Option<&[[f64; 2]]>) -> Option<Transition> {
        let containing = self.ordered.iter().find(|(index, grid)| {
            grid.bounds().contains(point[0], point[1])
                && !(*index == 0
                    && self
                        .patches
                        .iter()
                        .any(|bounds| bounds.interior(point[0], point[1])))
        });
        let radius = self.maximum.min(
            600.0_f64
                .max(containing.map_or(self.maximum, |(_, grid)| grid.terrain_step_meters) * 4.0),
        );
        let mut candidates = Vec::new();
        for (_, grid) in &self.ordered {
            let row_position = grid.rows as f64
                - 1.0
                - (point[0] - grid.north_min_meters) / grid.terrain_step_meters;
            let column_position = (point[1] - grid.east_min_meters) / grid.terrain_step_meters;
            let cell_radius = (radius / grid.terrain_step_meters).ceil() as isize;
            let center_row = (row_position + 0.5).floor() as isize;
            let center_column = (column_position + 0.5).floor() as isize;
            for row in center_row - cell_radius..=center_row + cell_radius {
                if row < 0 || row >= grid.rows as isize {
                    continue;
                }
                for column in center_column - cell_radius..=center_column + cell_radius {
                    if column < 0 || column >= grid.columns as isize {
                        continue;
                    }
                    let index = row as usize * grid.columns + column as usize;
                    if !grid.land_mask[index] || grid.elevation_meters_above_water[index].is_none()
                    {
                        continue;
                    }
                    let candidate = grid.point(row as usize, column as usize);
                    let distance = (candidate[0] - point[0]).hypot(candidate[1] - point[1]);
                    if distance > 1e-6
                        && distance <= radius
                        && self.mapped(candidate[0], candidate[1])
                        && ring.is_none_or(|ring| point_in_ring(candidate[0], candidate[1], ring))
                    {
                        candidates.push((candidate, distance));
                    }
                }
            }
        }
        candidates.sort_by(|first, second| first.1.total_cmp(&second.1));
        candidates.into_iter().find_map(|(candidate, width)| {
            self.surface(candidate[0], candidate[1])
                .map(|height| Transition {
                    direction: [
                        (candidate[0] - point[0]) / width,
                        (candidate[1] - point[1]) / width,
                    ],
                    width,
                    height,
                })
        })
    }

    fn append_path(&self, mesh: &mut Builder, points: &[[f64; 2]], closed: bool, open_side: f64) {
        if points.len() < 2 {
            return;
        }
        let area: f64 = points
            .iter()
            .zip(points.iter().cycle().skip(1))
            .take(points.len())
            .map(|(current, next)| current[1] * next[0] - next[1] * current[0])
            .sum();
        let side = if closed {
            if area < 0.0 { -1.0 } else { 1.0 }
        } else {
            open_side
        };
        let mut samples = Vec::new();
        for segment in 0..if closed {
            points.len()
        } else {
            points.len() - 1
        } {
            let start = points[segment];
            let end = points[(segment + 1) % points.len()];
            let length = (end[0] - start[0]).hypot(end[1] - start[1]);
            let divisions = (length / 30.0).ceil().max(1.0) as usize;
            for part in 0..divisions {
                let fraction = part as f64 / divisions as f64;
                samples.push([
                    start[0] + (end[0] - start[0]) * fraction,
                    start[1] + (end[1] - start[1]) * fraction,
                ]);
            }
        }
        if !closed {
            samples.push(*points.last().unwrap());
        }
        if samples.len() < 2 {
            return;
        }
        let mut normals = Vec::new();
        let mut transitions = Vec::new();
        for (index, &point) in samples.iter().enumerate() {
            let before = samples[if index == 0 {
                if closed { samples.len() - 1 } else { 0 }
            } else {
                index - 1
            }];
            let after = samples[if index == samples.len() - 1 {
                if closed { 0 } else { index }
            } else {
                index + 1
            }];
            let delta = [after[0] - before[0], after[1] - before[1]];
            let length = delta[0].hypot(delta[1]);
            let length = if length == 0.0 { 1.0 } else { length };
            let normal = [delta[1] / length * side, -delta[0] / length * side];
            normals.push(normal);
            let mut transition = self
                .find_land(point, normal)
                .or_else(|| self.find_land(point, normal.map(|value| -value)));
            let refine = closed || (index >= 8 && index + 8 < samples.len());
            if refine
                && transition.is_some_and(|transition| {
                    transition.width > 120.0_f64.max(self.minimum_step * 4.0)
                })
                && let Some(nearby) = self.nearby(point, closed.then_some(points))
                && transition.is_none_or(|transition| nearby.width < transition.width)
            {
                transition = Some(nearby);
            }
            transitions.push(transition);
        }
        for index in 0..samples.len() {
            if transitions[index].is_some() {
                continue;
            }
            let previous = (1..=9).find_map(|distance| {
                if !closed && distance > index {
                    None
                } else {
                    transitions[(index + samples.len() - distance % samples.len()) % samples.len()]
                        .map(|value| (distance, value))
                }
            });
            let next = (1..=9).find_map(|distance| {
                if !closed && index + distance >= samples.len() {
                    None
                } else {
                    transitions[(index + distance) % samples.len()].map(|value| (distance, value))
                }
            });
            let (Some((previous_distance, previous)), Some((next_distance, next))) =
                (previous, next)
            else {
                continue;
            };
            if previous_distance + next_distance - 1 > 8 {
                continue;
            }
            let fraction = previous_distance as f64 / (previous_distance + next_distance) as f64;
            let direction = if closed {
                normals[index]
            } else {
                std::array::from_fn(|axis| {
                    previous.direction[axis]
                        + (next.direction[axis] - previous.direction[axis]) * fraction
                })
            };
            let length = direction[0].hypot(direction[1]);
            if length < 0.5 {
                continue;
            }
            transitions[index] = Some(Transition {
                direction: direction.map(|value| value / length),
                width: previous.width + (next.width - previous.width) * fraction,
                height: previous.height + (next.height - previous.height) * fraction,
            });
        }
        if !closed {
            if let Some(first) = transitions
                .iter()
                .position(Option::is_some)
                .filter(|&index| index <= 8)
            {
                let reference = transitions[first].unwrap();
                for index in 0..first {
                    transitions[index] = Some(Transition {
                        direction: normals[index],
                        ..reference
                    });
                }
            }
            if let Some(last) = transitions
                .iter()
                .rposition(Option::is_some)
                .filter(|&index| transitions.len() - 1 - index <= 8)
            {
                let reference = transitions[last].unwrap();
                for index in last + 1..transitions.len() {
                    transitions[index] = Some(Transition {
                        direction: normals[index],
                        ..reference
                    });
                }
            }
            let supported = transitions.clone();
            for index in 0..transitions.len() {
                if transitions[index].is_some() {
                    continue;
                }
                let nearest = (1..=8).find_map(|distance| {
                    [
                        index.checked_sub(distance),
                        index
                            .checked_add(distance)
                            .filter(|&candidate| candidate < supported.len()),
                    ]
                    .into_iter()
                    .flatten()
                    .find_map(|candidate| supported[candidate])
                });
                if let Some(nearest) = nearest {
                    let normal = normals[index];
                    let sign = if nearest.direction[0] * normal[0]
                        + nearest.direction[1] * normal[1]
                        < 0.0
                    {
                        -1.0
                    } else {
                        1.0
                    };
                    transitions[index] = Some(Transition {
                        direction: normal.map(|value| value * sign),
                        ..nearest
                    });
                }
            }
        }
        for (index, transition) in transitions.iter_mut().enumerate() {
            let Some(current) = *transition else {
                continue;
            };
            let point = samples[index];
            let mut width = current.width + 5.0;
            let mut surface;
            loop {
                let north = point[0] + current.direction[0] * width;
                let east = point[1] + current.direction[1] * width;
                surface = if self.mapped(north, east) {
                    self.surface(north, east)
                } else {
                    None
                };
                if surface.is_some() || width >= self.maximum {
                    break;
                }
                width += self.increment;
            }
            if let Some(height) = surface {
                *transition = Some(Transition {
                    width,
                    height,
                    ..current
                });
            } else if !self.mapped(
                point[0] + current.direction[0] * current.width,
                point[1] + current.direction[1] * current.width,
            ) {
                *transition = Some(Transition {
                    height: -4.0,
                    ..current
                });
            }
        }
        let mut path = Vec::<(usize, u32)>::new();
        for (index, (&point, &transition)) in samples.iter().zip(&transitions).enumerate() {
            let Some(transition) = transition else {
                continue;
            };
            let height = transition.height - 0.15;
            let start = mesh.positions.len() as u32;
            for (ring, (offset, height)) in [
                (-240.0, -8.0),
                (0.0, -8.0),
                (transition.width * 0.5, height * 0.5),
                (transition.width, height),
            ]
            .into_iter()
            .enumerate()
            {
                let north = point[0] + transition.direction[0] * offset;
                let east = point[1] + transition.direction[1] * offset;
                mesh.append(
                    [east, height, -north],
                    terrain_color(
                        if ring == 3 { transition.height } else { height },
                        north,
                        east,
                    ),
                );
            }
            if let Some(&(previous_index, previous)) = path.last()
                && index == previous_index + 1
            {
                connect(mesh, previous, start);
            }
            path.push((index, start));
        }
        if closed
            && path.len() > 2
            && path.first().unwrap().0 == 0
            && path.last().unwrap().0 == samples.len() - 1
        {
            connect(mesh, path.last().unwrap().1, path.first().unwrap().1);
        }
    }
}

fn connect(mesh: &mut Builder, previous: u32, current: u32) {
    for ring in 0..3 {
        mesh.upward(previous + ring, current + ring, previous + ring + 1);
        mesh.upward(previous + ring + 1, current + ring, current + ring + 1);
    }
}

pub(super) fn generate(shore: &Shoreline, grids: &[Grid]) -> Result<VenueTerrainMesh, String> {
    let mut ordered: Vec<_> = grids
        .iter()
        .enumerate()
        .map(|(index, grid)| (index, with_fallback(grid, grids)))
        .collect();
    ordered.sort_by(|first, second| {
        first
            .1
            .terrain_step_meters
            .total_cmp(&second.1.terrain_step_meters)
    });
    let minimum_step = grids
        .iter()
        .map(|grid| grid.terrain_step_meters)
        .fold(f64::INFINITY, f64::min);
    let maximum_step = grids
        .iter()
        .map(|grid| grid.terrain_step_meters)
        .fold(0.0, f64::max);
    let context = Context {
        ordered,
        patches: grids[1..].iter().map(Grid::bounds).collect(),
        maximum: 2400.0_f64.min(120.0_f64.max(maximum_step * 8.0)),
        increment: 15.0_f64.min(5.0_f64.max(minimum_step * 0.5)),
        minimum_step,
    };
    let mut mesh = Builder::new();
    for (points, &side) in shore
        .shorelines_north_east_meters
        .iter()
        .zip(&shore.shoreline_land_side_signs)
    {
        context.append_path(&mut mesh, points, false, f64::from(side));
    }
    for island in &shore.islands {
        context.append_path(&mut mesh, &island.ring_north_east_meters, true, 1.0);
    }
    mesh.finish(
        "lake-biwa-shoreline-terrain-transition".into(),
        VenueTerrainMaterial::Terrain,
    )
}
