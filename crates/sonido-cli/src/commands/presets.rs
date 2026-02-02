//! Preset management commands.
//!
//! Provides commands to list, show, save, and manage effect presets.

use clap::{Args, Subcommand};
use sonido_config::{
    factory_presets, get_factory_preset,
    list_user_presets,
    user_presets_dir, ensure_user_presets_dir,
    Preset, EffectConfig,
};
use std::path::PathBuf;

#[derive(Args)]
pub struct PresetsArgs {
    #[command(subcommand)]
    command: PresetsCommand,
}

#[derive(Subcommand)]
enum PresetsCommand {
    /// List available presets (factory and user)
    List {
        /// Show only factory presets
        #[arg(long)]
        factory: bool,

        /// Show only user presets
        #[arg(long)]
        user: bool,
    },

    /// Show details of a preset
    Show {
        /// Preset name or path
        name: String,
    },

    /// Save current effect chain as a preset
    Save {
        /// Name for the new preset
        name: String,

        /// Effect chain specification (e.g., "distortion:drive=15|reverb:mix=0.5")
        #[arg(short, long)]
        chain: String,

        /// Description of the preset
        #[arg(short, long)]
        description: Option<String>,

        /// Overwrite if preset already exists
        #[arg(long)]
        force: bool,
    },

    /// Delete a user preset
    Delete {
        /// Preset name to delete
        name: String,

        /// Don't ask for confirmation
        #[arg(long)]
        force: bool,
    },

    /// Copy a factory preset to user presets for customization
    Copy {
        /// Factory preset name
        source: String,

        /// New preset name (optional, uses source name if not specified)
        #[arg(short, long)]
        name: Option<String>,
    },

    /// Show preset directories
    Paths,
}

pub fn run(args: PresetsArgs) -> anyhow::Result<()> {
    match args.command {
        PresetsCommand::List { factory, user } => list_presets(factory, user),
        PresetsCommand::Show { name } => show_preset(&name),
        PresetsCommand::Save { name, chain, description, force } => {
            save_preset(&name, &chain, description.as_deref(), force)
        }
        PresetsCommand::Delete { name, force } => delete_preset(&name, force),
        PresetsCommand::Copy { source, name } => copy_preset(&source, name.as_deref()),
        PresetsCommand::Paths => show_paths(),
    }
}

fn list_presets(factory_only: bool, user_only: bool) -> anyhow::Result<()> {
    let show_factory = !user_only;
    let show_user = !factory_only;

    if show_factory {
        println!("Factory Presets:");
        println!("================");
        for preset in factory_presets() {
            let desc = preset.description.as_deref().unwrap_or("");
            println!("  {:20} - {}", preset.name, desc);
        }
        println!();
    }

    if show_user {
        println!("User Presets:");
        println!("=============");
        let user_presets = list_user_presets();
        if user_presets.is_empty() {
            println!("  (none)");
            println!();
            println!("  Create a preset with: sonido presets save <name> --chain \"...\"\n");
        } else {
            for path in user_presets {
                let name = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");

                // Try to load and show description
                match Preset::load(&path) {
                    Ok(preset) => {
                        let desc = preset.description.as_deref().unwrap_or("");
                        println!("  {:20} - {}", name, desc);
                    }
                    Err(_) => {
                        println!("  {:20} - (error loading)", name);
                    }
                }
            }
        }
        println!();
    }

    Ok(())
}

fn show_preset(name: &str) -> anyhow::Result<()> {
    // Try to find the preset
    let preset = find_preset(name)?;

    println!("Preset: {}", preset.name);
    println!("{}", "=".repeat(8 + preset.name.len()));
    println!();

    if let Some(desc) = &preset.description {
        println!("Description: {}", desc);
        println!();
    }

    println!("Sample Rate: {} Hz", preset.sample_rate);
    println!();

    println!("Effects ({}):", preset.effects.len());
    for (i, effect) in preset.effects.iter().enumerate() {
        let bypass_marker = if effect.bypassed { " [BYPASSED]" } else { "" };
        println!("  {}. {}{}", i + 1, effect.effect_type, bypass_marker);

        if !effect.params.is_empty() {
            for (key, value) in &effect.params {
                println!("      {} = {}", key, value);
            }
        }
    }

    println!();

    // Show as chain specification
    let chain_spec = preset_to_chain_spec(&preset);
    println!("Chain specification:");
    println!("  {}", chain_spec);

    Ok(())
}

fn save_preset(name: &str, chain: &str, description: Option<&str>, force: bool) -> anyhow::Result<()> {
    // Ensure user presets directory exists
    ensure_user_presets_dir()?;

    let preset_path = user_presets_dir().join(format!("{}.toml", name));

    // Check if preset already exists
    if preset_path.exists() && !force {
        anyhow::bail!(
            "Preset '{}' already exists. Use --force to overwrite.",
            name
        );
    }

    // Parse chain specification and create preset
    let effects = parse_chain_to_configs(chain)?;

    let mut preset = Preset::new(name);
    if let Some(desc) = description {
        preset = preset.with_description(desc);
    }
    preset = preset.with_effects(effects);

    // Save the preset
    preset.save(&preset_path)?;

    println!("Saved preset '{}' to {}", name, preset_path.display());
    Ok(())
}

fn delete_preset(name: &str, force: bool) -> anyhow::Result<()> {
    // Don't allow deleting factory presets
    if get_factory_preset(name).is_some() {
        anyhow::bail!("Cannot delete factory preset '{}'. Factory presets are built-in.", name);
    }

    let preset_path = user_presets_dir().join(format!("{}.toml", name));

    if !preset_path.exists() {
        anyhow::bail!("User preset '{}' not found.", name);
    }

    if !force {
        // In a real implementation, we'd prompt for confirmation
        // For CLI simplicity, we require --force
        anyhow::bail!(
            "Use --force to confirm deletion of preset '{}'.",
            name
        );
    }

    std::fs::remove_file(&preset_path)?;
    println!("Deleted preset '{}'.", name);

    Ok(())
}

fn copy_preset(source: &str, new_name: Option<&str>) -> anyhow::Result<()> {
    // Get the factory preset
    let preset = get_factory_preset(source)
        .ok_or_else(|| anyhow::anyhow!("Factory preset '{}' not found.", source))?;

    let target_name = new_name.unwrap_or(source);

    // Ensure user presets directory exists
    ensure_user_presets_dir()?;

    let preset_path = user_presets_dir().join(format!("{}.toml", target_name));

    if preset_path.exists() {
        anyhow::bail!(
            "Preset '{}' already exists in user presets. Choose a different name with --name.",
            target_name
        );
    }

    // Create a new preset with the target name
    let mut new_preset = Preset::new(target_name);
    if let Some(desc) = &preset.description {
        new_preset = new_preset.with_description(format!("{} (copy)", desc));
    }
    new_preset = new_preset
        .with_sample_rate(preset.sample_rate)
        .with_effects(preset.effects.clone());

    new_preset.save(&preset_path)?;

    println!(
        "Copied factory preset '{}' to user preset '{}'",
        source, target_name
    );
    println!("Path: {}", preset_path.display());

    Ok(())
}

fn show_paths() -> anyhow::Result<()> {
    println!("Preset Directories:");
    println!("===================");
    println!();
    println!("User presets:   {}", user_presets_dir().display());
    println!("System presets: {}", sonido_config::system_presets_dir().display());
    println!("Config dir:     {}", sonido_config::user_config_dir().display());

    Ok(())
}

// Helper functions

fn find_preset(name: &str) -> anyhow::Result<Preset> {
    // Check if it's a path
    let path = PathBuf::from(name);
    if path.exists() {
        return Preset::load(&path).map_err(|e| anyhow::anyhow!("{}", e));
    }

    // Try factory preset
    if let Some(preset) = get_factory_preset(name) {
        return Ok(preset);
    }

    // Try user preset
    let user_path = user_presets_dir().join(format!("{}.toml", name));
    if user_path.exists() {
        return Preset::load(&user_path).map_err(|e| anyhow::anyhow!("{}", e));
    }

    // Try system preset
    if let Some(path) = sonido_config::find_preset(name) {
        return Preset::load(&path).map_err(|e| anyhow::anyhow!("{}", e));
    }

    anyhow::bail!("Preset '{}' not found.", name)
}

fn preset_to_chain_spec(preset: &Preset) -> String {
    preset
        .effects
        .iter()
        .map(|effect| {
            let mut spec = effect.effect_type.clone();
            if effect.bypassed {
                spec.push('!');
            }
            if !effect.params.is_empty() {
                let params: Vec<String> = effect
                    .params
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect();
                spec.push(':');
                spec.push_str(&params.join(","));
            }
            spec
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn parse_chain_to_configs(chain: &str) -> anyhow::Result<Vec<EffectConfig>> {
    let mut effects = Vec::new();

    for effect_spec in chain.split('|') {
        let effect_spec = effect_spec.trim();
        if effect_spec.is_empty() {
            continue;
        }

        // Check for bypass suffix
        let (spec, bypassed) = if effect_spec.ends_with('!') {
            (&effect_spec[..effect_spec.len() - 1], true)
        } else {
            (effect_spec, false)
        };

        // Split into name and params
        let parts: Vec<&str> = spec.splitn(2, ':').collect();
        let effect_type = parts[0].trim();

        let mut config = EffectConfig::new(effect_type).with_bypass(bypassed);

        // Parse parameters
        if parts.len() > 1 {
            for param in parts[1].split(',') {
                let param = param.trim();
                if param.is_empty() {
                    continue;
                }

                let kv: Vec<&str> = param.splitn(2, '=').collect();
                if kv.len() != 2 {
                    anyhow::bail!("Invalid parameter format: '{}' (expected key=value)", param);
                }

                config = config.with_param(kv[0].trim(), kv[1].trim());
            }
        }

        effects.push(config);
    }

    Ok(effects)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_chain_simple() {
        let configs = parse_chain_to_configs("distortion").unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].effect_type, "distortion");
        assert!(!configs[0].bypassed);
    }

    #[test]
    fn test_parse_chain_with_params() {
        let configs = parse_chain_to_configs("distortion:drive=15,tone=4000").unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].get_param("drive"), Some("15"));
        assert_eq!(configs[0].get_param("tone"), Some("4000"));
    }

    #[test]
    fn test_parse_chain_multiple() {
        let configs = parse_chain_to_configs("preamp:gain=6|distortion:drive=12|reverb").unwrap();
        assert_eq!(configs.len(), 3);
        assert_eq!(configs[0].effect_type, "preamp");
        assert_eq!(configs[1].effect_type, "distortion");
        assert_eq!(configs[2].effect_type, "reverb");
    }

    #[test]
    fn test_parse_chain_bypassed() {
        let configs = parse_chain_to_configs("distortion!|reverb").unwrap();
        assert_eq!(configs.len(), 2);
        assert!(configs[0].bypassed);
        assert!(!configs[1].bypassed);
    }
}
