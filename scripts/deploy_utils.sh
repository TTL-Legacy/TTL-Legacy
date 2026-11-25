#!/usr/bin/env bash
# Shared utility functions for deployment scripts

# Parse contract address from environments.toml for given network
get_contract_address() {
  local network=$1
  grep -A 1 "\[${network}\]" environments.toml | grep "contract_ttl_vault" | cut -d'"' -f2
}

# Update contract address in environments.toml for given network
set_contract_address() {
  local network=$1
  local contract_id=$2
  local temp_file=$(mktemp)

  awk -v net="$network" -v cid="$contract_id" '
    BEGIN { found = 0 }
    /^\['"$network"'\]/ { found = 1; print; next }
    found && /^contract_ttl_vault/ { print "contract_ttl_vault = \"" cid "\""; found = 0; next }
    found && /^\[/ { found = 0 }
    { print }
  ' environments.toml > "$temp_file"

  mv "$temp_file" environments.toml
}

# Prompt user for confirmation (returns 0 if yes, 1 if no)
confirm() {
  local prompt=$1
  local response
  read -p "$prompt (yes/no): " response
  [[ "$response" =~ ^[Yy][Ee][Ss]$ ]]
}
