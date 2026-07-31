use std::io;

fn main() -> io::Result<()> {
    let mut stdout = io::stdout();
    yps_dap::serve(io::stdin(), &mut stdout)
}
