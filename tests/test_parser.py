# tests/test_parser.py - Test dependency file parsing
import tempfile
import pytest
from core.parser import parse_dependency_file, parse_requirements_txt, parse_environment_yml
from pathlib import Path

class TestDependencyParser:
    """Test dependency file parsing functionality."""
    
    def test_parse_requirements_txt(self, sample_requirements_txt):
        """Test parsing requirements.txt files."""
        packages = parse_dependency_file(sample_requirements_txt)
        
        # Check that packages were parsed
        assert len(packages) > 0
        
        # Check specific packages
        package_names = [pkg['name'] for pkg in packages]
        assert 'requests' in package_names
        assert 'django' in package_names
        assert 'flask' in package_names
        assert 'numpy' in package_names
        
        # Check version parsing
        requests_pkg = next(pkg for pkg in packages if pkg['name'] == 'requests')
        assert requests_pkg['version'] == '2.25.1'
        
        # Check packages without versions
        numpy_pkg = next(pkg for pkg in packages if pkg['name'] == 'numpy')
        assert numpy_pkg['version'] is None
    
    def test_parse_environment_yml(self, sample_environment_yml):
        """Test parsing environment.yml files."""
        packages = parse_dependency_file(sample_environment_yml)
        
        # Check that packages were parsed
        assert len(packages) > 0
        
        package_names = [pkg['name'] for pkg in packages]
        assert 'requests' in package_names
        assert 'django' in package_names
        assert 'flask' in package_names
        assert 'numpy' in package_names
    
    def test_empty_file(self):
        """Test handling empty files."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.txt', delete=False) as f:
            f.write("")
            f.flush()
            
            packages = parse_dependency_file(f.name)
            assert packages == []
            
        Path(f.name).unlink()
    
    def test_comments_and_options(self, sample_requirements_txt):
        """Test that comments and pip options are ignored."""
        packages = parse_dependency_file(sample_requirements_txt)
        
        # Should not include comments or options
        package_names = [pkg['name'] for pkg in packages]
        assert 'other-requirements.txt' not in str(packages)
        assert 'comment' not in str(packages).lower()
