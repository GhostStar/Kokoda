param([Parameter(Mandatory=$true)][string]$StateFile,[Parameter(Mandatory=$true)][string]$CommandFile)
Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing @'
using System; using System.IO; using System.Windows.Forms; using System.Drawing;
public class SharingFixture : Form {
 public static string StatePath,CommandPath;
 Form dialog, sibling; Timer timer=new Timer(); string previous="";
 public SharingFixture() {
  Text="Kokoda Public Root";ClientSize=new Size(400,260);
  Controls.Add(new Label{Text="Synthetic shared window",Dock=DockStyle.Fill});
  dialog=new Form{Text="Kokoda Owned Dialog",Size=new Size(240,180)};
  sibling=new Form{Text="Kokoda Private Sibling",Size=new Size(260,180)};
  Shown+=(s,e)=>{dialog.Show(this);sibling.Show();Publish();timer.Start();};
  timer.Interval=50;timer.Tick+=(s,e)=>{
   string command=File.Exists(CommandPath)?File.ReadAllText(CommandPath).Trim():"";
   if(command==previous)return;previous=command;
   if(command=="revoke")Text="Kokoda Private Root";
   if(command=="restore")Text="Kokoda Public Root";
   if(command=="detach")dialog.Owner=null;
   Publish();
  };
  FormClosing+=(s,e)=>{timer.Stop();dialog.Close();sibling.Close();};
 }
 void Publish(){File.WriteAllText(StatePath,"{\"root\":"+Handle.ToInt64()+",\"dialog\":"+dialog.Handle.ToInt64()+",\"sibling\":"+sibling.Handle.ToInt64()+",\"command\":\""+previous+"\"}");}
 public static void Run(){Application.Run(new SharingFixture());}
}
'@
[SharingFixture]::StatePath=$StateFile
[SharingFixture]::CommandPath=$CommandFile
[SharingFixture]::Run()
