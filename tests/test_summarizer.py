# tests/test_summarizer.py - Test output formatting
import pytest
from unittest.mock import patch
from core.summarizer import Summary

class TestSummaryFormatter:
    """Test the output formatting functionality."""
    
    def test_console_formatter_initialization(self):
        """Test console formatter initialization."""
        formatter = Summary(output_format='console')
        assert formatter.output_format == 'console'
        assert 'CRITICAL' in formatter.colors
        assert 'HIGH' in formatter.colors
    
    def test_json_formatter_initialization(self):
        """Test JSON formatter initialization."""
        formatter = Summary(output_format='json')
        assert formatter.output_format == 'json'
    
    def test_vulnerability_extraction_from_string(self):
        """Test extracting vulnerabilities from text."""
        formatter = Summary()
        
        sample_text = """
        Security Analysis Results:
        CVE-2023-1234 found in requests package
        GHSA-abcd-efgh-ijkl is CRITICAL severity
        Another GHSA-xyz9-8765-4321 marked as HIGH severity
        """
        
        vulns = formatter._parse_vulnerabilities_from_text(sample_text)
        
        # Should extract vulnerability IDs
        assert len(vulns) >= 1
        vuln_ids = [v.get('id', '') for v in vulns]
        assert any('CVE-2023-1234' in vid for vid in vuln_ids)
    
    @patch('builtins.print')
    def test_console_display(self, mock_print, sample_vulnerabilities):
        """Test console output display."""
        formatter = Summary(output_format='console')
        dependencies = [
            {"name": "requests", "version": "2.25.1"},
            {"name": "django", "version": "2.1.0"}
        ]
        
        # Mock agent result with vulnerabilities
        mock_result = "Mock security analysis result"
        
        # This should not crash
        formatter.display_results(mock_result, dependencies)
        
        # Print should have been called
        assert mock_print.called