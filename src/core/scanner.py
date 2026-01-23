# src/utils/scanner.py
from packaging import version
from typing import List

def is_version_affected(package_version: str, affected_ranges: List[str]) -> bool:
    """
    Check if a package_version falls within any of the affected_ranges.
    (Simplified logic for PoC)
    """
    if not affected_ranges:
        return True 
    
    try:
        current_version = version.parse(package_version)
        
        is_affected = False
        for constraint_str in affected_ranges:
            if not constraint_str:
                continue
            
            if constraint_str.startswith("=="):
                if current_version == version.parse(constraint_str[2:]): is_affected = True; break
            elif constraint_str.startswith("!="):
                if current_version != version.parse(constraint_str[2:]): is_affected = True # Keep checking other constraints
            elif constraint_str.startswith(">="):
                if current_version >= version.parse(constraint_str[2:]): is_affected = True
            elif constraint_str.startswith("<="):
                if current_version <= version.parse(constraint_str[2:]): is_affected = True
            elif constraint_str.startswith(">"):
                if current_version > version.parse(constraint_str[1:]): is_affected = True
            elif constraint_str.startswith("<"):
                if current_version < version.parse(constraint_str[1:]): is_affected = True
            elif version.parse(constraint_str) == current_version: # Exact match without operator
                is_affected = True; break
        
        return is_affected
    except Exception as e:
        print(f"Error parsing version or range '{package_version}' vs '{affected_ranges}': {e}")
        return False 

def get_severity_from_score(base_score: float) -> str:
    """
    Convert a CVSS base score to a severity rating.
    """
    try:
        score = float(base_score)
        if score >= 9.0: return "CRITICAL"
        elif score >= 7.0: return "HIGH"
        elif score >= 4.0: return "MEDIUM"
        elif score > 0.0: return "LOW"
        else: return "UNKNOWN"
    except (ValueError, TypeError):
        return "UNKNOWN"