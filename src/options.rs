use getopts::{Fail, Matches, Options};

// $ ace run <options>

pub fn get_runner_options(args: Vec<String>) -> Result<Matches, Fail> {
    let mut opts = Options::new();

    opts.optopt("n", "name", "set container name", "CONTAINER_NAME");
    opts.optopt("", "exec", "exec command", "COMMAND");
    opts.optflag("h", "help", "print help message");
    opts.optflag("", "del", "delete container");

    opts.parse(&args[1..])
}

// $ ace network <options>

pub fn get_network_options(args: Vec<String>) -> Result<Matches, Fail> {
    let mut opts = Options::new();

    opts.optflag("", "create-brige", "create ace0 bridge");
    opts.optflag("", "delete-brige", "delete ace0 bridge");

    opts.parse(&args[1..])
}
