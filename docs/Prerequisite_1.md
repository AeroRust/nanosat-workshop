# Prerequisite - Chapter 1

1. Install Rust using `rustup` - [https://www.rust-lang.org/tools/install](rust-install)
2. Setup git - you will also need to set your `ssh` key in Github to:
   - [Generating a new SSH key and adding it to the ssh-agent (Github)][github-generate-ssh-key]
   - [Adding a new SSH key to your GitHub account][github-add-key-to-account]
3. Clone repository: `git clone git@github.com:LechevSpace/nanosat-workshop.git`
4. Install RiscV toolchain: `rustup target install riscv32imac-unknown-none-elf`
5. Build the application: `cd nanosat-workshop && cargo build`

[rust-install]: https://www.rust-lang.org/tools/install
[github-generate-ssh-key]: https://docs.github.com/en/authentication/connecting-to-github-with-ssh/generating-a-new-ssh-key-and-adding-it-to-the-ssh-agent
[github-add-key-to-account]: https://docs.github.com/en/authentication/connecting-to-github-with-ssh/adding-a-new-ssh-key-to-your-github-account