# app.py
import os
import json
import sys
import yaml
import argparse
from pathlib import Path
from smolagents import CodeAgent, InferenceClientModel

# imports utils
from core.parser import parse_dependency_file
from src.utils.spinner import run_with_spinner
from core.summarizer import Summarizer

from src.tools.final_answer import FinalAnswerTool
from src.tools.package_scan import PackageScanTool
from src.tools.pypi_tool import PypiTool
from src.tools.github_tool import GithubTool

def get_hf_token():
    """Get HF_TOKEN from environment variables."""
    return os.getenv("HF_TOKEN")

def load_config():
    """Load agent configuration from agent.json."""
    try:
        with open("agent.json", 'r') as f:
            config = json.load(f)
        return config
    except FileNotFoundError:
        print("❌ Error: agent.json not found. Please ensure it exists in the root directory.")
        sys.exit(1)
    except json.JSONDecodeError:
        print("❌ Error: agent.json is malformed (invalid JSON). Please check its syntax.")
        sys.exit(1)

def load_prompts():
    """Load prompt templates from prompts.yaml."""
    try:
        with open("prompts.yaml", 'r') as f:
            prompts = yaml.safe_load(f)
        
        if not isinstance(prompts, dict) or 'system_prompt' not in prompts:
            print("❌ Error: prompts.yaml is malformed or missing the 'system_prompt' key.")
            sys.exit(1)
            
        return prompts
        
    except FileNotFoundError:
        print("❌ Error: prompts.yaml not found. Please ensure it exists in the root directory.")
        sys.exit(1)
    except yaml.YAMLError as e:
        print(f"❌ Error parsing prompts.yaml: {e}. Please check its YAML syntax.")
        sys.exit(1)

def initialize_tools():
    """Initialize all necessary tool instances for the agent."""
    tools = []
    tools.append(PackageScanTool())
    tools.append(GithubTool())
    tools.append(PypiTool())
    tools.append(FinalAnswerTool())
    return tools

def create_security_agent(model_id=None):
    """Create and configure the security analysis agent."""
    
    config = load_config()
    prompts = load_prompts()
    tools = initialize_tools()
    
    # Extract model configuration
    model_config = config.get("model", {}).get("data", {})
    if model_id is None:
        model_id = model_config.get("model_id", 'Qwen/Qwen2.5-Coder-32B-Instruct')

    model = InferenceClientModel(
        max_tokens=model_config.get("max_tokens", 3072),
        temperature=model_config.get("temperature", 0.2),
        model_id=model_id,
        custom_role_conversions=model_config.get("custom_role_conversions", None),
    )

    # Agent parameters
    agent_params = {
        "model": model,
        "tools": tools,
        "max_steps": config.get("max_steps", 8),
        "verbosity_level": config.get("verbosity_level", 2),
        "grammar": config.get("grammar", None),
        "planning_interval": config.get("planning_interval", 3),
        "name": config.get("name", "PythonSecurityAnalyst"),
        "description": config.get("description", "Expert cybersecurity agent for Python package security analysis"),
        "prompt_templates": prompts,
    }

    agent = CodeAgent(**agent_params)
    return agent

def scan_command(file_path: str, output_format: str = 'console', model_id: str = None):
    """Scan command with spinner integration and error handling."""
    
    try:
        print(f"🎯 Scanning: {file_path}")
        print(f"📊 Output format: {output_format}")
        print()
        
        # Step 1: Parse dependencies with spinner
        def parse_dependencies():
            return parse_dependency_file(file_path)
        
        dependencies = run_with_spinner(
            parse_dependencies,
            f"Parsing {Path(file_path).name}",
            "Dependencies parsed successfully"
        )
        
        if not dependencies:
            print("❌ No packages found to scan")
            return False
        
        # Show number of packages found
        print(f"✓ Found {len(dependencies)} packages to scan")
        
        # Show packages being scanned
        print("\n📋 Packages identified:")
        for i, pkg in enumerate(dependencies[:5]):
            version_info = f" (v{pkg['version']})" if pkg.get('version') else ""
            print(f"   {i+1}. {pkg['name']}{version_info}")
        if len(dependencies) > 5:
            print(f"   ... and {len(dependencies) - 5} more packages")
        print()
        
        # Step 2: Create security agent with spinner
        def setup_agent():
            return create_security_agent(model_id)
        
        agent = run_with_spinner(
            setup_agent,
            "Initializing AI security agent",
            "Security agent ready"
        )
        
        if not agent:
            print("❌ Failed to create security agent")
            return False
        
        # Step 3: Run comprehensive security analysis with spinner
        def run_security_analysis():
            task = f"""
            Perform a security vulnerability scan of these {len(dependencies)} Python packages:

            {chr(10).join([f"- {pkg['name']}" + (f" version {pkg['version']}" if pkg.get('version') else " (latest)") for pkg in dependencies])}

            Instructions:
            1. Use the package_scan tool to scan the specified packages for vulnerabilities
            2. Focus on Critical and High severity vulnerabilities
            3. Identify any potentially malicious packages
            4. Provide specific remediation recommendations
            5. Give an overall security assessment

            Please provide a detailed security report with:
            - Executive summary of findings
            - List of vulnerable packages with severity levels
            - Specific upgrade recommendations
            - Overall risk assessment

            Be thorough but concise in your analysis.
            """
            
            return agent.run(task)
        
        result = run_with_spinner(
            run_security_analysis,
            "Running comprehensive security analysis",
            "Security analysis completed"
        )
        
        # Step 4: Display results
        print()
        if result:
            formatter = Summarizer(output_format=output_format)
            formatter.display_results(result, dependencies)
        else:
            print("❌ No analysis results were returned by the security agent")
            return False
            
        print("\n✅ Scan completed successfully!")
        return True
        
    except FileNotFoundError:
        print(f"❌ File not found: {file_path}")
        return False
    except Exception as e:
        print(f"❌ Scan failed: {e}")
        print(f"Error details: {type(e).__name__}: {str(e)}")
        return False

def main():
    """Main application entry point."""

    # Parse command line arguments FIRST (so --help works)
    parser = argparse.ArgumentParser(
        description="🔒 MotionStream - AI-Powered Python Security Scanner",
        epilog="""
            Examples:
            motionstream scan requirements.txt
            motionstream scan environment.yml --output json
            motionstream scan requirements.txt --output html
            Or for custom model
            motionstream scan requirements.txt
            motionstream scan environment.yml --output json
            motionstream scan requirements.txt --output html
            motionstream scan requirements.txt --model "model-id"
            motionstream scan requirements.txt --model "model-id" --output json
        """,
        formatter_class=argparse.RawDescriptionHelpFormatter
    )
    
    parser.add_argument('command', choices=['scan'], help='Command to execute')
    parser.add_argument('file_path', help='Path to requirements.txt or environment.yml')
    parser.add_argument('--output', choices=['console', 'json', 'html'], 
                       default='console', help='Output format (default: console)')
    
    args = parser.parse_args()
    
    # Check HF_TOKEN only when actually needed for scanning
    if args.command == 'scan':
        if not get_hf_token():
            print("❌ Error: HF_TOKEN environment variable not set.")
            print("   Set it with: export HF_TOKEN='your_huggingface_token'")
            print("   Get a token at: https://huggingface.co/settings/tokens")
            sys.exit(1)
            
        success = scan_command(args.file_path, args.output)
        sys.exit(0 if success else 1)

if __name__ == "__main__":
    main()