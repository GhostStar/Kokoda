param([string]$ResultFile = "$env:TEMP\Kokoda-image-input.json")
Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing @'
using System;
using System.IO;
using System.Drawing;
using System.Drawing.Imaging;
using System.Windows.Forms;
public class ImageClipboardFixture : Form {
 public static string ResultPath;
 DataObject backup=new DataObject(); bool empty; Bitmap picture;
 PictureBox view=new PictureBox {Dock=DockStyle.Fill,SizeMode=PictureBoxSizeMode.Zoom};
 public ImageClipboardFixture() {
  Text="Kokoda Image Clipboard Test";ClientSize=new Size(500,350);KeyPreview=true;Controls.Add(view);
  var original=Clipboard.GetDataObject();empty=original==null;
  if(original!=null) foreach(var format in original.GetFormats(false)) {
   var data=original.GetData(format,false);if(data!=null) backup.SetData(format,false,data);
  }
  File.WriteAllText(ResultPath,"{}");
  KeyDown+=(s,e)=> {
   if(e.Control && e.KeyCode==Keys.V) {Paste();e.SuppressKeyPress=true;}
   if(e.Control && e.KeyCode==Keys.C) {Copy(true);e.SuppressKeyPress=true;}
   if(e.KeyCode==Keys.F7) {var bad=new DataObject();bad.SetData("PNG",false,new MemoryStream(new byte[]{1,2,3}));bad.SetData("Kokoda.ImageFixture",false,"1");Clipboard.SetDataObject(bad,true);e.SuppressKeyPress=true;}
   if(e.KeyCode==Keys.F8) {Copy(false);e.SuppressKeyPress=true;}
   if(e.KeyCode==Keys.F9) {Copy(true);e.SuppressKeyPress=true;}
  };
  FormClosing+=(s,e)=> {
   if(Clipboard.ContainsData("Kokoda.ImageFixture") || IsFixtureImage()) {
    if(empty) Clipboard.Clear();else Clipboard.SetDataObject(backup,true);
   }
   if(picture!=null)picture.Dispose();
  };
 }
 bool IsFixtureImage() {
  try {using(var img=Clipboard.GetImage()) {if(img==null || img.Width!=2 || img.Height!=2)return false;using(var b=new Bitmap(img)){var c=b.GetPixel(0,0);return c.R>250 && c.G<5 && c.B<5;}}} catch{return false;}
 }
 void Paste() {
  Bitmap next=null;
  var data=Clipboard.GetDataObject();var png=data.GetData("PNG",false) as MemoryStream;
  if(png!=null) {png.Position=0;using(var image=Image.FromStream(png))next=new Bitmap(image);}
  else if(Clipboard.ContainsImage()) {using(var image=Clipboard.GetImage())next=new Bitmap(image);}
  if(next==null)return;
  if(picture!=null)picture.Dispose();picture=next;view.Image=picture;
  var values=new System.Collections.Generic.List<string>();
  for(int y=0;y<picture.Height;y++)for(int x=0;x<picture.Width;x++) {var c=picture.GetPixel(x,y);values.Add("["+c.R+","+c.G+","+c.B+","+c.A+"]");}
  File.WriteAllText(ResultPath,"{\"width\":"+picture.Width+",\"height\":"+picture.Height+",\"pixels\":["+string.Join(",",values)+"]}");
 }
 void Copy(bool png) {
  if(picture==null)return;
  var data=new DataObject();
  data.SetData("Kokoda.ImageFixture",false,"1");
  if(png) {var stream=new MemoryStream();picture.Save(stream,ImageFormat.Png);stream.Position=0;data.SetData("PNG",false,stream);}
  data.SetData(DataFormats.Bitmap,true,new Bitmap(picture));
  Clipboard.SetDataObject(data,true);
 }
 public static void Run(){Application.Run(new ImageClipboardFixture());}
}
'@
[ImageClipboardFixture]::ResultPath=$ResultFile
[ImageClipboardFixture]::Run()
