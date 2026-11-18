# Test helper: opens an .mpp in the installed scheduling program (hidden) and saves the
# program's own XML view next to it as FILE.ref.xml.
param([string]$File)
$app = New-Object -ComObject MSProject.Application
$app.Visible = $false
$app.DisplayAlerts = $false
try {
    $app.FileOpenEx($File, $true) | Out-Null
    $ref = [System.IO.Path]::ChangeExtension($File, ".ref.xml")
    [void]$app.GetType().InvokeMember("FileSaveAs", [System.Reflection.BindingFlags]::InvokeMethod, $null, $app, @($ref, "MSProject.XML"), $null, $null, [string[]]@("Name", "FormatID"))
    $app.FileCloseEx(0) | Out-Null
    Write-Output "saved $ref"
} finally {
    $app.Quit(0)
    [System.Runtime.InteropServices.Marshal]::ReleaseComObject($app) | Out-Null
}
