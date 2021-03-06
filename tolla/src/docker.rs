use shiplift::Docker;
use shiplift::builder::ContainerOptionsBuilder;
use shiplift::builder::RmContainerOptions;
use shiplift::rep::Container;
use url::Url;
use std::collections::HashMap;
use std::fs::DirBuilder;
use std::fs::File;
use std::io::Write;
use bytes::BytesMut;

pub struct StoreManager {
    deamon: Docker,
}

impl StoreManager {
    // Connect to the docker deamon
    pub fn new(host: &String) -> StoreManager {
        let url = Url::parse(host).unwrap();
        println!("{}", url);
        let deamon = Docker::new();
        StoreManager { deamon: deamon }
    }

    // Start containers by id
    pub fn start_containers(&self, ids: Vec<String>) -> Result<(), String> {

        info!("starting containers");
        for container_id in ids {
            //let container = containers.get(&container_id);
            //if let Err(err) = container.start() {
            //    error!("{}", err.to_string());
            //    return Err(err.to_string());
            //}
            info!("Successfully started {}", container_id);
        }

        Ok(())
    }

    // Retrive container by name
    fn container_by_id(&self, id: &String) -> Result<Option<Container>, String> {
        let containers = self.deamon.containers();

        let containers = containers.list(&Default::default()).map_err(
            |e| e.to_string(),
        )?;

        let matches: Vec<_> = containers
            .into_iter()
            .filter(|container: &Container| container.Names.contains(&id))
            .collect();

        if matches.len() == 0 {
            return Ok(None);
        }
        Ok(Some(matches[0].clone()))
    }

    // check container id exists
    pub fn verify_container_id(&self, id: &String) -> Result<bool, String> {
        match self.container_by_id(id) {
            Err(err) => return Err(err.to_string()),
            Ok(container) => {
                return Ok(container.is_some());
            }
        }
    }

    // Creates a directory from which the container can read its content
    pub fn new_mountdir(
        &self,
        contents: HashMap<&str, &mut BytesMut>,
        dirname: &String,
    ) -> Result<(), String> {
        info!("Creating dir {}", dirname);
        if let Err(err) = DirBuilder::new().recursive(true).create(dirname) {
            return Err(err.to_string());
        }

        for (filename, content) in &contents {
            let mut file = match File::create(format!("{}/{}", dirname, filename)) {
                Ok(file) => file,
                Err(err) => return Err(err.to_string()),
            };
            info!("Creating file {}/{}", dirname, filename);

            if let Err(err) = file.write_all(content) {
                return Err(err.to_string());
            }
        }

        Ok(())
    }

    pub fn remove_container(&self, id: &String) -> Result<(), String> {
        let containers = self.deamon.containers();
        let container = containers.get(id);

        let rm_opts = RmContainerOptions::builder().force(true).build();

        container.remove(rm_opts).map_err(|e| e.to_string())
    }

    // Create a new container from image with name, and starts it.
    // The function returns the IPAddress on success.
    pub fn new_container(
        &self,
        image: &str,
        name: &str,
        env: Vec<String>,
    ) -> Result<(String, String), String> {
        let containers = self.deamon.containers();

        let mut opts = ContainerOptionsBuilder::new(image);

        let v2: Vec<&str> = env.iter().map(|s| &**s).collect();

        opts.env(v2);
        opts.name(name);

        //opts.volumes(volumes.clone().to_vec());
        opts.volumes_from(vec!["tolla"]);

        if let Err(err) = containers.create(&opts.build()) {
            error!("{}", err.to_string());
            return Err(err.to_string());
        }

        let container = containers.get(name);
        if let Err(err) = container.start() {
            error!("{}", err.to_string());
            return Err(err.to_string());
        }

        let id = container.id();

        // Read ipaddress of container
        let info = match container.inspect() {
            Ok(info) => info,
            Err(err) => return Err(err.to_string()),
        };

        info!(
            "successfully created container: {}:{}",
            id,
            info.NetworkSettings.IPAddress
        );
        Ok((String::from(id), info.NetworkSettings.IPAddress))
    }
}
