precommit:
    cargo sqlx prepare
    cargo fmt --all -- --check
    cargo clippy --fix --allow-dirty --allow-staged
    cargo machete
    cargo test

delete_db:
    -rm "/Users/shaankhosla/Library/Application Support/repeat/cards.db"
    -touch "/Users/shaankhosla/Library/Application Support/repeat/cards.db"
    DATABASE_URL="sqlite:///Users/shaankhosla/Library/Application Support/repeat/cards.db" sqlx migrate run

create:
    cargo run -- create /Users/shaankhosla/Desktop/sample_repeat_cards/test.md

check:
    cargo run -- check /Users/shaankhosla/Desktop/sample_repeat_cards/

drill:
    cargo run -- drill /Users/shaankhosla/Desktop/sample_repeat_cards/

release:
    just precommit
    version=$(rg --max-count 1 '^version = ' Cargo.toml | sed -E 's/version = "(.+)"/\1/')
    if [ -z "$version" ]; then
        echo "Unable to detect package version from Cargo.toml" >&2
        exit 1
    fi
    git cliff --config cliff.toml --tag v$version --unreleased --output CHANGELOG.md
    git add Cargo.toml Cargo.lock CHANGELOG.md
    git commit -m "chore(release): v$version"
    git tag -a v$version -m "v$version"
    git push origin HEAD
    git push origin v$version
