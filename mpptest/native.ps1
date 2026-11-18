# Test-file maker: builds a plan directly in the installed scheduling program (hidden) and
# saves it as .mpp plus the program's own XML view, to check the .mpp reader on a natural file.
param([string]$Dir)
$app = New-Object -ComObject MSProject.Application
$app.Visible = $false
$app.DisplayAlerts = $false
try {
    # a hidden instance has no new-file window: open a small plan and empty it first
    $seed = Join-Path $Dir "features.xml"
    $app.FileOpenEx($seed, $false, [Type]::Missing, [Type]::Missing, [Type]::Missing, [Type]::Missing, [Type]::Missing, [Type]::Missing, [Type]::Missing, "MSProject.XML") | Out-Null
    $p = $app.ActiveProject
    for ($i = $p.Tasks.Count; $i -ge 1; $i--) { $p.Tasks.Item($i).Delete() }
    for ($i = $p.Resources.Count; $i -ge 1; $i--) { $p.Resources.Item($i).Delete() }
    try { $p.Title = "Native Plan" } catch {}
    $names = @("Launch", "Research", "Interviews", "Write brief", "Brief approved", "Build", "Prototype", "Pilot", "Go live")
    foreach ($n in $names) { [void]$p.Tasks.Add($n) }
    # outline: 1 summary; 2 summary (3-5); 6 summary (7-8); 9 milestone under 1
    foreach ($id in 2..9) { $p.Tasks.Item($id).OutlineIndent() }
    foreach ($id in 3, 4, 5, 7, 8) { $p.Tasks.Item($id).OutlineIndent() }
    $p.Tasks.Item(3).Duration = "4d"
    $p.Tasks.Item(4).Duration = "3d"
    $p.Tasks.Item(4).Predecessors = "3"
    $p.Tasks.Item(5).Duration = "0d"
    $p.Tasks.Item(5).Predecessors = "4"
    $p.Tasks.Item(7).Duration = "2w"
    $p.Tasks.Item(7).Predecessors = "5FS+2d"
    $p.Tasks.Item(8).Duration = "6d"
    $p.Tasks.Item(8).Predecessors = "7SS+3d"
    $p.Tasks.Item(9).Duration = "0d"
    $p.Tasks.Item(9).Predecessors = "8"
    $p.Tasks.Item(8).Priority = 800
    $p.Tasks.Item(8).Notes = "Pilot with two teams"
    $p.Tasks.Item(9).Deadline = [datetime]::new(2027, 1, 29, 17, 0, 0)
    $r1 = $p.Resources.Add("Avery")
    $r1.StandardRate = "50/h"
    $r1.MaxUnits = 1
    $r2 = $p.Resources.Add("Contractors")
    $r2.MaxUnits = 3
    $r2.StandardRate = "80/h"
    $r3 = $p.Resources.Add("Travel")
    $r3.Type = 2
    $p.Tasks.Item(3).ResourceNames = "Avery"
    $p.Tasks.Item(4).ResourceNames = "Avery[50%]"
    $p.Tasks.Item(7).ResourceNames = "Contractors[200%]"
    $p.Tasks.Item(8).ResourceNames = "Contractors,Travel"
    $p.Tasks.Item(3).PercentComplete = 100
    $p.Tasks.Item(4).PercentComplete = 50
    $out = Join-Path $Dir "native.mpp"
    $app.FileSaveAs($out, 0) | Out-Null
    $ref = Join-Path $Dir "native.ref.xml"
    [void]$app.GetType().InvokeMember("FileSaveAs", [System.Reflection.BindingFlags]::InvokeMethod, $null, $app, @($ref, "MSProject.XML"), $null, $null, [string[]]@("Name", "FormatID"))
    $app.FileCloseEx(0) | Out-Null
    Write-Output "saved $out"
} finally {
    $app.Quit(0)
    [System.Runtime.InteropServices.Marshal]::ReleaseComObject($app) | Out-Null
}
