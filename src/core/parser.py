# src/utils/parser.py - Simple File Parser

import re
import yaml
from pathlib import Path
from typing import List, Dict, Any

def parse_dependency_file(file_path: str) -> List[Dict[str, Any]]:
    """
    Simple parser for requirements.txt and environment.yml files.
    Returns list of {'name': str, 'version': str} dictionaries.
    """
    file_path = Path(file_path)
    
    if file_path.suffix in ['.yml', '.yaml']:
        return parse_environment_yml(file_path)
    else:
        return parse_requirements_txt(file_path)

def parse_requirements_txt(file_path: Path) -> List[Dict[str, Any]]:
    """Parse requirements.txt - simple version."""
    packages = []
    
    with open(file_path, 'r') as f:
        for line in f:
            line = line.strip()
            
            # Skip comments and empty lines
            if not line or line.startswith('#') or line.startswith('-'):
                continue
            
            # Simple regex to extract package name and version
            match = re.match(r'^([a-zA-Z0-9_-]+)([>=<!=~]+)?([\d.]+)?', line)
            if match:
                name = match.group(1)
                version = match.group(3) if match.group(3) else None
                packages.append({"name": name, "version": version})
    
    return packages

def parse_environment_yml(file_path: Path) -> List[Dict[str, Any]]:
    """Parse environment.yml - simple version."""
    packages = []
    
    with open(file_path, 'r') as f:
        env_data = yaml.safe_load(f)
    
    # Get conda dependencies
    for dep in env_data.get('dependencies', []):
        if isinstance(dep, str):
            # Parse "package=version" or "package>=version"
            match = re.match(r'^([a-zA-Z0-9_-]+)([>=<!=~]+)?([\d.]+)?', dep)
            if match:
                name = match.group(1)
                version = match.group(3) if match.group(3) else None
                packages.append({"name": name, "version": version})
        
        elif isinstance(dep, dict) and 'pip' in dep:
            # Handle pip dependencies in conda environment
            for pip_dep in dep['pip']:
                match = re.match(r'^([a-zA-Z0-9_-]+)([>=<!=~]+)?([\d.]+)?', pip_dep)
                if match:
                    name = match.group(1)
                    version = match.group(3) if match.group(3) else None
                    packages.append({"name": name, "version": version})
    
    return packages