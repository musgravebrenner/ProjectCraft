# Test-file maker: opens each XML plan in the installed scheduling program (hidden) and saves
# it as .mpp, so the .mpp reader can be checked against files with known contents.
param([string]$Dir)
$app = New-Object -ComObject MSProject.Application
$app.Visible = $false
$app.DisplayAlerts = $false
try {
    foreach ($f in @(Get-ChildItem -Path $Dir -Filter *.xml | Where-Object { $_.Name -notlike "*.ref.xml" })) {
        $out = [System.IO.Path]::ChangeExtension($f.FullName, ".mpp")
        $app.FileOpenEx($f.FullName, $false, [Type]::Missing, [Type]::Missing, [Type]::Missing, [Type]::Missing, [Type]::Missing, [Type]::Missing, [Type]::Missing, "MSProject.XML") | Out-Null
        # XML import drops % complete; set it from the XML so the test files carry progress
        $x = [xml](Get-Content -Raw $f.FullName)
        foreach ($t in $x.Project.Tasks.Task) {
            if ([double]$t.PercentComplete -gt 0 -and $t.Summary -eq "0") {
                $task = $app.ActiveProject.Tasks.UniqueID([int]$t.UID)
                if ($task) { $task.PercentComplete = [int][double]$t.PercentComplete }
            }
        }
        $app.FileSaveAs($out, 0) | Out-Null   # 0 = pjMPP
        # the program's own view of the plan, as ground truth for the reader
        $ref = [System.IO.Path]::ChangeExtension($f.FullName, ".ref.txt")
        $ref = [System.IO.Path]::ChangeExtension($f.FullName, ".ref.xml")
        [void]$app.GetType().InvokeMember("FileSaveAs", [System.Reflection.BindingFlags]::InvokeMethod, $null, $app, @($ref, "MSProject.XML"), $null, $null, [string[]]@("Name", "FormatID"))
        $app.FileCloseEx(0) | Out-Null        # 0 = pjDoNotSave
        Write-Output "saved $out"
    }
} finally {
    $app.Quit(0)
    [System.Runtime.InteropServices.Marshal]::ReleaseComObject($app) | Out-Null
}
