# tests/test_integration.py - Integration tests
import pytest
from unittest.mock import Mock, patch
import tempfile
from pathlib import Path

class TestIntegration:
    """Integration tests for the complete workflow."""
    
    @patch('src.tools.package_scan.requests.post')
    def test_end_to_end_requirements_scan(self, mock_post, sample_requirements_txt):
        """Test complete workflow from file to results."""
        # Mock OSV API response
        mock_response = Mock()
        mock_response.raise_for_status.return_value = None
        mock_response.json.return_value = {
            "results": [
                {"vulns": [{"id": "GHSA-test-vuln", "modified": "2023-01-01T00:00:00Z"}]},
                {"vulns": []},
                {"vulns": []},
                {"vulns": []}
            ]
        }
        mock_post.return_value = mock_response
        
        # Test the parsing -> scanning -> formatting pipeline
        from core.parser import parse_dependency_file
        from src.tools.package_scan import PackageScanTool
        from core.summarizer import Summary
        
        # Parse dependencies
        dependencies = parse_dependency_file(sample_requirements_txt)
        assert len(dependencies) > 0
        
        # Scan for vulnerabilities
        scanner = PackageScanTool()
        vulnerabilities = scanner.forward(dependencies)
        assert len(vulnerabilities) == 1
        assert vulnerabilities[0]["vulnerability_id"] == "GHSA-test-vuln"
        
        # Format results
        formatter = Summary(output_format='json')
        # Should not crash
        formatter.display_results("Mock analysis", dependencies)