@{
    Severity     = @('Error', 'Warning')
    ExcludeRules = @(
        # Internal engine functions aren't cmdlets; -WhatIf support is not useful here
        'PSUseShouldProcessForStateChangingFunctions',
        # Get-Manifests / Get-Overrides etc. deliberately return collections
        'PSUseSingularNouns',
        # tools/ scripts are interactive; Write-Host is the right sink there
        'PSAvoidUsingWriteHost'
    )
}
