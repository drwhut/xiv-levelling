use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::io;

#[derive(Deserialize, Debug)]
#[serde(transparent)]
struct ServerList {
    servers: Vec<String>
}

impl ServerList {
    fn exists(&self, name: &str) -> bool {
        self.servers.contains(&name.to_owned())
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct Pagination {
    pub results: u8
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct PlayerSearchEntry {
    #[serde(rename = "ID")]
    pub id: u32,
    pub name: String
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct PlayerSearchResult {
    pub pagination: Pagination,
    pub results: Vec<PlayerSearchEntry>
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct ClassUnlockedState {
    pub name: String
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct ClassJob {
    #[serde(rename = "ClassID")]
    pub class_id: u8,
    pub level: u8,
    unlocked_state: ClassUnlockedState
}

impl ClassJob {
    fn name(&self) -> &str {
        &self.unlocked_state.name
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct PlayerCharacter {
    pub class_jobs: Vec<ClassJob>,
    pub name: String
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct CharacterMeta {
    pub character: PlayerCharacter
}

const TANK: [u8; 4] = [1, 3, 32, 37];
const HEALER: [u8; 3] = [6, 26, 33];
const DPS: [u8; 10] = [2, 4, 29, 34, 5, 31, 38, 7, 26, 35];

#[derive(Copy, Clone, Eq, PartialEq)]
struct PartyConfig {
    pub index: [usize; 4],
    pub var: u32,
    pub avg: u32
}

impl Ord for PartyConfig {
    fn cmp(&self, other: &Self) -> Ordering {
        other.var.cmp(&self.var)
    }
}

impl PartialOrd for PartyConfig {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(other.var.cmp(&self.var))
    }
}

fn main() {
    println!("Getting list of FFXIV servers...");
    let server_list = reqwest::blocking::get("https://xivapi.com/servers").unwrap()
        .json::<ServerList>().unwrap();
    
    let mut server_name = String::new();

    while !server_list.exists(&server_name) {
        server_name.clear();

        println!("Please enter the name of your FFXIV server:");
        io::stdin().read_line(&mut server_name).unwrap();

        server_name = server_name.trim().to_owned();

        if !server_list.exists(&server_name) {
            println!("Server {} does not exist!", server_name);
        }
    }

    let mut party: Vec<PlayerCharacter> = Vec::new();

    let mut character_name = " ".to_owned();
    while !character_name.is_empty() && party.len() < 4 {
        character_name.clear();

        println!("Character {} Name (press enter to stop):", party.len() + 1);
        io::stdin().read_line(&mut character_name).unwrap();

        character_name = character_name.trim().to_owned();

        if !character_name.is_empty() {
            println!("Searching for {} in the Lodestone...", character_name);
            let player_search = reqwest::blocking::get(format!("https://xivapi.com/character/search?name={}&server={}", character_name, server_name)).unwrap()
                .json::<PlayerSearchResult>().unwrap();
            
            if player_search.pagination.results == 1 {
                let search_result = &player_search.results[0];
                println!("Found character {} with ID {}!", search_result.name, search_result.id);

                println!("Getting character data for {}...", search_result.name);

                let mut character_meta = reqwest::blocking::get(format!("https://xivapi.com/character/{}", search_result.id)).unwrap()
                    .json::<CharacterMeta>().unwrap();
                
                character_meta.character.class_jobs.retain(|x| TANK.contains(&x.class_id) || HEALER.contains(&x.class_id) || DPS.contains(&x.class_id));
                party.push(character_meta.character);

            } else if player_search.pagination.results == 0 {
                println!("No character with that name was found!");
            } else {
                println!("Multiple characters were found!");
            }
        }
    }

    if party.len() < 2 {
        println!("Party must consist of at least two characters!");
        return;
    }

    println!("Determining best possible party configurations for levelling...\n");
    let mut party_configs: BinaryHeap<PartyConfig> = BinaryHeap::new();

    let mut combination = vec![];
    for _ in 0..party.len() {
        combination.push(0);
    }

    loop {
        let mut num_tanks = 0;
        let mut num_healers = 0;
        let mut all_max = true;
        let mut all_unlocked = true;

        for i in 0..combination.len() {
            let class_job = &party[i].class_jobs[combination[i]];

            if TANK.contains(&class_job.class_id) {
                num_tanks += 1;
            } else if HEALER.contains(&class_job.class_id) {
                num_healers += 1;
            }

            if class_job.level == 0 {
                all_unlocked = false;
            } else if class_job.level < 80 {
                all_max = false;
            }
        }

        if num_tanks == 1 && num_healers == 1 && all_unlocked && !all_max {
            let mut index = [0, 0, 0, 0];
            let mut var = 0;
            let mut avg = 0;

            for i in 0..combination.len() {
                index[i] = combination[i];

                let job1 = &party[i].class_jobs[combination[i]];
                for j in 0..combination.len() {
                    if i != j {
                        let job2 = &party[j].class_jobs[combination[j]];
                        var += (job1.level as i16 - job2.level as i16).abs() as u32;
                    }
                }

                avg += job1.level as u32;
            }

            avg /= combination.len() as u32;

            party_configs.push(PartyConfig {
                index: index,
                var: var,
                avg: avg
            })
        }

        for i in (0..combination.len()).rev() {
            combination[i] += 1;
            if combination[i] >= party[i].class_jobs.len() {
                combination[i] = 0;
            } else {
                break;
            }
        }

        let mut back_to_start = true;
        for i in 0..combination.len() {
            if combination[i] > 0 {
                back_to_start = false;
                break;
            }
        }

        if back_to_start {
            break;
        }
    }

    let mut input = String::new();
    while !(input.eq("q") || party_configs.is_empty()) {
        if let Some(party_config) = party_configs.pop() {
            for i in 0..party.len() {
                let class_job = &party[i].class_jobs[party_config.index[i]];
                println!("{0: <20}: {1: <15} Lv {2}", party[i].name, class_job.name(), class_job.level);
            }
            println!("- Lv Var: {}", party_config.var);
            println!("- Lv Avg: {}", party_config.avg);
        }

        input.clear();
        io::stdin().read_line(&mut input).unwrap();
        input = input.trim().to_owned();
    }
}
