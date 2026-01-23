# src/utils/summarizer.py
import re
import json
from typing import Dict, List, Any
from datetime import datetime
from pathlib import Path

class Summarizer:
    """Format security scan results for different output types."""
    
    def __init__(self, output_format: str = 'console'):
        self.output_format = output_format
        self.colors = {
            'CRITICAL': '\033[91m',    # Red
            'HIGH': '\033[93m',        # Yellow
            'MEDIUM': '\033[94m',      # Blue
            'LOW': '\033[92m',         # Green
            'UNKNOWN': '\033[90m',     # Gray
            'RESET': '\033[0m',        # Reset
            'BOLD': '\033[1m',         # Bold
            'WARNING': '\033[93m',     # Yellow
            'SUCCESS': '\033[92m',     # Green
            'INFO': '\033[96m',        # Cyan
            'ORANGE': '\033[38;5;208m',   # Orange
        }
    
    def display_results(self, agent_result: Any, dependencies: List[Dict[str, Any]]):
        """Display results in the specified format."""
        if self.output_format == 'console':
            self._display_console(agent_result, dependencies)
        elif self.output_format == 'json':
            self._display_json(agent_result, dependencies)
        elif self.output_format == 'html':
            self._display_html(agent_result, dependencies)
    
    def _display_console(self, agent_result: Any, dependencies: List[Dict[str, Any]]):
        """Display results in security scanner console format."""
        
        print(f"\n🔒 {self.colors['INFO']}MotionStream Security Report{self.colors['RESET']}")
        print()
        
        # Parse vulnerabilities from agent result
        vulnerabilities = self._extract_vulnerabilities_from_agent_result(agent_result)
        
        # Create vulnerability lookup
        vuln_lookup = {v['package']: v for v in vulnerabilities}
        
        print(f"📦 Scanned {len(dependencies)} packages:")
        
        # Display each package with status
        for dep in dependencies:
            if not dep.get('name'):
                continue
                
            package_name = dep['name']
            version = dep.get('version', 'latest')
            
            if package_name in vuln_lookup:
                vuln = vuln_lookup[package_name]
                severity = vuln.get('severity', 'UNKNOWN')
                if severity == 'CRITICAL':
                    print(f"   ❌ {package_name} {self.colors['ORANGE']}{version}{self.colors['RESET']} - {self.colors['CRITICAL']}CRITICAL{self.colors['RESET']} vulnerabilities found")
                elif severity == 'HIGH':
                    print(f"   ❌ {package_name} {self.colors['ORANGE']}{version}{self.colors['RESET']} - {self.colors['HIGH']}HIGH{self.colors['RESET']} vulnerabilities found")
                else:
                    print(f"   ❌ {package_name} {self.colors['ORANGE']}{version}{self.colors['RESET']} - vulnerabilities found")
            else:
                print(f"   ✓ {package_name} {self.colors['ORANGE']}{version}{self.colors['RESET']}")
        
        print()
        
        # Display detailed security issues
        if vulnerabilities:
            print(f"🔍 {self.colors['INFO']}Security Issues Found:{self.colors['RESET']}")
            print()
            
            for vuln in vulnerabilities:
                severity = vuln.get('severity', 'UNKNOWN')
                package = vuln.get('package', 'unknown')
                version = vuln.get('version', 'latest')
                component = vuln.get('component', 'package')
                fixed_version = vuln.get('fixed_version', 'latest')
                
                severity_color = self.colors.get(severity, self.colors['RESET'])
                
                if severity == 'CRITICAL':
                    print(f"⚠ {severity_color}{severity}{self.colors['RESET']}: {package} {self.colors['ORANGE']}{version}{self.colors['RESET']} has remote code execution vulnerability")
                    print(f"  Impact: {component} compromise")
                elif severity == 'HIGH':
                    print(f"⚠ {severity_color}{severity}{self.colors['RESET']}: {package} {self.colors['ORANGE']}{version}{self.colors['RESET']} has file access vulnerabilities")
                    print(f"  Impact: {component}")
                else:
                    print(f"⚠ {severity_color}{severity}{self.colors['RESET']}: {package} {self.colors['ORANGE']}{version}{self.colors['RESET']} has security issues")
                    print(f"  Impact: {component}")
                
                print(f"  Fix: {self.colors['SUCCESS']}pip install {package}>={fixed_version}{self.colors['RESET']}")
                print()
        
        # Summary statistics
        critical_count = len([v for v in vulnerabilities if v.get('severity') == 'CRITICAL'])
        high_count = len([v for v in vulnerabilities if v.get('severity') == 'HIGH'])
        total_vulns = len(vulnerabilities)
        
        print(f"📊 Summary: {total_vulns} vulnerabilities found", end="")
        if critical_count > 0 or high_count > 0:
            print(f" ({critical_count} Critical, {high_count} High)")
        else:
            print()
            
        if total_vulns > 0:
            print(f"🎯 Recommendation: {self.colors['WARNING']}Update vulnerable packages immediately{self.colors['RESET']}")
        else:
            print(f"🎯 Status: {self.colors['SUCCESS']}All packages are secure{self.colors['RESET']}")
    
    def _display_json(self, agent_result: Any, dependencies: List[Dict[str, Any]]):
        """Display results in JSON format."""
        vulnerabilities = self._extract_vulnerabilities_from_agent_result(agent_result)
        
        report = {
            'scan_timestamp': datetime.now().isoformat(),
            'dependencies_scanned': len(dependencies),
            'vulnerabilities_found': len(vulnerabilities),
            'summary': {
                'critical': len([v for v in vulnerabilities if v.get('severity') == 'CRITICAL']),
                'high': len([v for v in vulnerabilities if v.get('severity') == 'HIGH']),
                'medium': len([v for v in vulnerabilities if v.get('severity') == 'MEDIUM']),
                'low': len([v for v in vulnerabilities if v.get('severity') == 'LOW']),
            },
            'dependencies': dependencies,
            'vulnerabilities': vulnerabilities,
            'agent_analysis': str(agent_result) if agent_result else None
        }
        
        print(json.dumps(report, indent=2, default=str))
    
    def _display_html(self, agent_result: Any, dependencies: List[Dict[str, Any]]):
        """Display results in HTML format."""
        vulnerabilities = self._extract_vulnerabilities_from_agent_result(agent_result)
        
        html_report = f"""
        <!DOCTYPE html>
        <html>
        <head>
            <title>Security Scan Report</title>
            <style>
                body {{ font-family: Arial, sans-serif; margin: 20px; }}
                .critical {{ color: #d32f2f; }}
                .high {{ color: #f57c00; }}
                .medium {{ color: #1976d2; }}
                .low {{ color: #388e3c; }}
                .summary {{ background: #f5f5f5; padding: 15px; border-radius: 5px; }}
                .vulnerability {{ border: 1px solid #ddd; margin: 10px 0; padding: 10px; }}
            </style>
        </head>
        <body>
            <h1>🔒 Security Scan Report</h1>
            <div class="summary">
                <h2>Summary</h2>
                <p>Dependencies Scanned: {len(dependencies)}</p>
                <p>Vulnerabilities Found: {len(vulnerabilities)}</p>
            </div>
            
            <h2>Vulnerabilities</h2>
        """
        
        for vuln in vulnerabilities:
            severity_class = vuln.get('severity', 'unknown').lower()
            html_report += f"""
            <div class="vulnerability">
                <h3 class="{severity_class}">{vuln.get('package')} - {vuln.get('severity', 'UNKNOWN')}</h3>
                <p><strong>CVE:</strong> {vuln.get('id', 'N/A')}</p>
                <p><strong>Description:</strong> {vuln.get('summary', 'No description available')}</p>
                <p><strong>Fixed Version:</strong> {vuln.get('fixed_version', 'Check documentation')}</p>
            </div>
            """
        
        html_report += """
        </body>
        </html>
        """
        
        # Save to file
        report_file = Path(f"security_report_{datetime.now().strftime('%Y%m%d_%H%M%S')}.html")
        with open(report_file, 'w') as f:
            f.write(html_report)
        
        print(f"HTML report saved to: {report_file}")

    def _extract_vulnerabilities_from_agent_result(self, agent_result: Any) -> List[Dict[str, Any]]:
        """
        Extract vulnerability information from agent result.
        """
        vulnerabilities = []
        
        if isinstance(agent_result, dict) and 'vulnerable_packages' in agent_result:
            vulnerable_packages = agent_result['vulnerable_packages']
            
            for package_name, package_vulns in vulnerable_packages.items():
                if isinstance(package_vulns, list):
                    for vuln in package_vulns:
                        vulnerabilities.append({
                            'package': package_name,
                            'severity': vuln.get('severity', 'UNKNOWN').upper(),
                            'id': vuln.get('vulnerability_id', 'N/A'),
                            'summary': f'Security vulnerability in {package_name}',
                            'fixed_version': 'latest',
                            'component': 'package',
                            'version': 'current'
                        })
        
        return vulnerabilities

    def _parse_vulnerabilities_from_text(self, text: str) -> List[Dict[str, Any]]:
        """Parse vulnerability information from text output."""
        # This is a simple parser - you can make it more sophisticated
        vulnerabilities = []
        
        # Look for common vulnerability patterns in the text
        lines = text.split('\n')
        current_vuln = {}
        
        for line in lines:
            line = line.strip()
            if 'CVE-' in line or 'GHSA-' in line:
                if current_vuln:
                    vulnerabilities.append(current_vuln)
                current_vuln = {'id': line}
            elif any(severity in line.upper() for severity in ['CRITICAL', 'HIGH', 'MEDIUM', 'LOW']):
                for severity in ['CRITICAL', 'HIGH', 'MEDIUM', 'LOW']:
                    if severity in line.upper():
                        current_vuln['severity'] = severity
                        break
        
        if current_vuln:
            vulnerabilities.append(current_vuln)
        
        return vulnerabilities