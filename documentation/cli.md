# Usage

```sh
# global options
dots --config path/to/config.kdl
dots -vvv # increase verbosity (can be used multiple times)
dots --dry-run

# doctor
## arguments
dots doctor BUNDLE_NAME... # default to 'all' if no bundle names provided
## options
dots doctor --help
## commands
dots doctor # implies 'dots dependencies check' and 'dots dotfiles status'

# install
## arguments
dots install BUNDLE_NAME... # default to 'all' if no bundle names provided
## options
dots install --help
## commands
dots install # implies 'dots dependencies install' and 'dots dotfiles link'

# dependencies
## arguments
dots dependencies COMMAND BUNDLE_NAME... # default to 'all' if no bundle names provided
## options
dots dependencies --help
## commands
dots dependencies list
dots dependencies install
dots dependencies doctor

# dotfiles
## arguments
dots dotfiles COMMAND BUNDLE_NAME... # default to 'all' if no bundle names provided
## options
dots dotfiles --help
## commands
dots dotfiles list
dots dotfiles install
dots dotfiles doctor
```
