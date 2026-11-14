//! Native code for both phones from the same project:
//! * Android in Kotlin: the same layout XML, an Activity per screen.
//! * iPhone in Swift: SwiftUI, a view and a model per screen.
//! Each block has a Kotlin and a Swift form here, next to its Java form in blocks.rs.

use crate::codegen::{indent, quote, sanitize, Gen, Lang};
use crate::model::*;

/// Kotlin for a block: {0}.. arguments, {S1}/{S2} inner stacks, {R} loop counter.
/// Numbers are Doubles, as in the engine.
pub fn kotlin_tpl(op: &str) -> Option<&'static str> {
    Some(match op {
        // Variable
        "setVarBoolean" | "setVarInt" | "setVarString" => "{0} = {1}",
        "increaseInt" => "{0}++",
        "decreaseInt" => "{0}--",
        "mapCreateNew" => "{0} = HashMap()",
        "mapPut" => "{0}[{1}] = {2}",
        "mapGet" => "({0}[{1}]?.toString() ?: \"\")",
        "mapContainKey" => "{0}.containsKey({1})",
        "mapRemoveKey" => "{0}.remove({1})",
        "mapSize" => "{0}.size.toDouble()",
        "mapClear" => "{0}.clear()",
        "mapIsEmpty" => "{0}.isEmpty()",
        "getVar" => "{0}",
        "eventPosition" => "_position",
        "eventChecked" => "_isChecked",
        "eventText" => "_charSeq",
        "eventProgress" => "_progressValue",
        // List
        "addListInt" | "addListStr" | "addMapToList" => "{1}.add({0})",
        "insertListInt" | "insertListStr" => "{2}.add(({1}).toInt(), {0})",
        "getAtListInt" | "getAtListStr" => "{1}[({0}).toInt()]",
        "indexListInt" | "indexListStr" => "{1}.indexOf({0}).toDouble()",
        "containListInt" | "containListStr" => "{0}.contains({1})",
        "getMapInList" => "{2} = {1}[({0}).toInt()]",
        "getAtListMap" => "({2}[({0}).toInt()][{1}]?.toString() ?: \"\")",
        "lengthList" => "{0}.size.toDouble()",
        "deleteList" => "{1}.removeAt(({0}).toInt())",
        "clearList" => "{0}.clear()",
        // Control
        "repeat" => "for ({R} in 0 until ({0}).toInt()) {\n{S1}\n}",
        "forever" => "while (true) {\n{S1}\n}",
        "whileLoop" => "while ({0}) {\n{S1}\n}",
        "if" => "if ({0}) {\n{S1}\n}",
        "ifElse" => "if ({0}) {\n{S1}\n}\nelse {\n{S2}\n}",
        "break" => "break",
        // Operator
        "true" => "true",
        "false" => "false",
        "<" => "{0} < {1}",
        "=" => "{0} == {1}",
        ">" => "{0} > {1}",
        "&&" => "{0} && {1}",
        "||" => "{0} || {1}",
        "not" => "!{0}",
        "+" => "{0} + {1}",
        "-" => "{0} - {1}",
        "*" => "{0} * {1}",
        "/" => "{0} / {1}",
        "%" => "{0} % {1}",
        "random" => "AppUtil.randomBetween(({0}).toInt(), ({1}).toInt()).toDouble()",
        "stringLength" => "{0}.length.toDouble()",
        "stringJoin" => "{0} + {1}",
        "stringIndex" => "{1}.indexOf({0}).toDouble()",
        "stringLastIndex" => "{1}.lastIndexOf({0}).toDouble()",
        "stringSub" => "{0}.substring(({1}).toInt(), ({2}).toInt())",
        "stringEquals" => "{0} == {1}",
        "stringContains" => "{0}.contains({1})",
        "stringReplace" => "{0}.replace({1}, {2})",
        "trim" => "{0}.trim()",
        "toUpperCase" => "{0}.uppercase()",
        "toLowerCase" => "{0}.lowercase()",
        "toNumber" => "{0}.toDouble()",
        "toString" => "({0}).toLong().toString()",
        "toStringWithDecimal" => "({0}).toString()",
        // Math
        "mathGetDip" => "AppUtil.dpToPx(applicationContext, ({0}).toInt()).toDouble()",
        "mathGetDisplayWidth" => "AppUtil.screenWidth(applicationContext).toDouble()",
        "mathGetDisplayHeight" => "AppUtil.screenHeight(applicationContext).toDouble()",
        "mathPi" => "Math.PI",
        "mathE" => "Math.E",
        "mathPow" => "Math.pow({0}, {1})",
        "mathMin" => "Math.min({0}, {1})",
        "mathMax" => "Math.max({0}, {1})",
        "mathSqrt" => "Math.sqrt({0})",
        "mathAbs" => "Math.abs({0})",
        "mathRound" => "Math.round({0}).toDouble()",
        "mathCeil" => "Math.ceil({0})",
        "mathFloor" => "Math.floor({0})",
        "mathSin" => "Math.sin({0})",
        "mathCos" => "Math.cos({0})",
        "mathTan" => "Math.tan({0})",
        "mathLog" => "Math.log({0})",
        "mathLog10" => "Math.log10({0})",
        "mathToRadian" => "Math.toRadians({0})",
        "mathToDegree" => "Math.toDegrees({0})",
        // File
        "fileGetData" => "({0}.getString({1}, \"\") ?: \"\")",
        "fileSetData" => "{0}.edit().putString({1}, {2}).commit()",
        "fileRemoveData" => "{0}.edit().remove({1}).commit()",
        "fileContainsData" => "{0}.contains({1})",
        // View
        "setEnable" => "{0}.isEnabled = {1}",
        "getEnable" => "{0}.isEnabled",
        "setVisible" => "{0}.visibility = View.{1}",
        "setBgColor" => "AppUtil.setBg({0}, {1})",
        "setRadius" => "AppUtil.setRadius({0}, {1})",
        "setElevation" => "AppUtil.setElevation({0}, {1})",
        "setBorder" => "AppUtil.setBorder({0}, {1}, {2})",
        "setAlpha" => "{0}.alpha = ({1}).toFloat()",
        "setText" => "{0}.setText({1})",
        "getText" => "{0}.text.toString()",
        "setTextColor" => "{0}.setTextColor({1})",
        "setTextSize" => "{0}.textSize = ({1}).toFloat()",
        "setFont" => "AppUtil.setFont({0}, {1})",
        "setVideo" | "setVideoFrom" => "{0}_video.load({1})",
        "videoPlay" => "{0}_video.play()",
        "videoPause" => "{0}_video.pause()",
        "videoSeek" => "{0}_video.seek({1})",
        "videoLoop" => "{0}_video.setLooping({1})",
        "videoIsPlaying" => "{0}_video.isPlaying",
        "videoPosition" => "{0}_video.position",
        "videoDuration" => "{0}_video.duration",
        "assetText" => "AppUtil.assetText(_activity, {0})",
        "setHint" => "{0}.hint = {1}",
        "setChecked" => "{0}.isChecked = {1}",
        "getChecked" => "{0}.isChecked",
        "seekBarSetProgress" => "{0}.progress = ({1}).toInt()",
        "seekBarGetProgress" => "{0}.progress.toDouble()",
        "seekBarSetMax" => "{0}.max = ({1}).toInt()",
        "seekBarGetMax" => "{0}.max.toDouble()",
        "listSetData" => "{0}.adapter = ArrayAdapter(baseContext, android.R.layout.simple_list_item_1, {1})",
        "webViewLoadUrl" => "{0}.loadUrl({1})",
        "setImage" | "setImageFrom" => "AppUtil.setPicture({0}, {1})",
        "soundPlay" | "soundPlayFrom" => "{0}.play(_activity, {1})",
        "cameraPhoto" => "AppMedia.photo(_activity) { _ok, _filePath ->\nif (_ok) {\n{S1}\n} else {\n{S2}\n}\n}",
        "cameraVideo" => "AppMedia.video(_activity) { _ok, _filePath ->\nif (_ok) {\n{S1}\n} else {\n{S2}\n}\n}",
        "pickMedia" => "AppMedia.pick(_activity, {0}) { _ok, _filePath ->\nif (_ok) {\n{S1}\n} else {\n{S2}\n}\n}",
        "mediaPath" => "_filePath",
        "soundPause" => "{0}.pause()",
        "soundResume" => "{0}.resume()",
        "soundStop" => "{0}.stop()",
        "soundLoop" => "{0}.setLooping({1})",
        "soundVolume" => "{0}.setVolume({1})",
        "soundSeek" => "{0}.seek({1})",
        "soundIsPlaying" => "{0}.isPlaying",
        "soundPosition" => "{0}.position",
        "soundDuration" => "{0}.duration",
        "requestFocus" => "{0}.requestFocus()",
        "performClick" => "{0}.performClick()",
        // Component
        "doToast" => "AppUtil.toast(applicationContext, {0})",
        "setTitle" => "title = {0}",
        "copyToClipboard" => "(getSystemService(Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager).setPrimaryClip(android.content.ClipData.newPlainText(\"clipboard\", {0}))",
        "finishActivity" => "finish()",
        "intentSetScreen" => "{0}.setClass(applicationContext, {1}::class.java)",
        "intentPutExtra" => "{0}.putExtra({1}, {2})",
        "intentGetString" => "(intent.getStringExtra({0}) ?: \"\")",
        "startActivity" => "startActivity({0})",
        "openScreen" => "startActivity(android.content.Intent(applicationContext, {0}::class.java))",
        "timerAfter" => "{0}?.cancel()\n{0} = object : TimerTask() {\noverride fun run() {\nrunOnUiThread {\n{S1}\n}\n}\n}\n_timer.schedule({0}!!, ({1}).toLong())",
        "timerEvery" => "{0}?.cancel()\n{0} = object : TimerTask() {\noverride fun run() {\nrunOnUiThread {\n{S1}\n}\n}\n}\n_timer.scheduleAtFixedRate({0}!!, ({1}).toLong(), ({2}).toLong())",
        "timerCancel" => "{0}?.cancel()",
        "dialogSetTitle" => "{0}.setTitle({1})",
        "dialogSetMessage" => "{0}.setMessage({1})",
        "dialogOkButton" => "{0}.setPositiveButton({1}) { _, _ ->\n{S1}\n}",
        "dialogCancelButton" => "{0}.setNegativeButton({1}) { _, _ ->\n{S1}\n}",
        "dialogShow" => "{0}.create().show()",
        "vibratorAction" => "{0}.vibrate(({1}).toLong())",
        "calendarGetNow" => "{0} = Calendar.getInstance()",
        "calendarFormat" => "SimpleDateFormat({1}).format({0}.time)",
        "calendarGetTime" => "{0}.timeInMillis.toDouble()",
        "supaSignUp" => "Supa.auth(_activity, \"signup\", {0}, {1}) { _ok, _code, _response ->\nif (_ok) {\n{S1}\n} else {\n{S2}\n}\n}",
        "supaSignIn" => "Supa.auth(_activity, \"token?grant_type=password\", {0}, {1}) { _ok, _code, _response ->\nif (_ok) {\n{S1}\n} else {\n{S2}\n}\n}",
        "supaSignOut" => "Supa.signOut(_activity)",
        "supaSignedIn" => "Supa.signedIn(_activity)",
        "supaUserEmail" => "Supa.email(_activity)",
        "supaUserId" => "Supa.userId(_activity)",
        "supaSelect" => "Supa.rest(_activity, \"GET\", {0}, \"\", \"\", \"\") { _ok, _code, _response ->\nif (_ok) {\n{S1}\n} else {\n{S2}\n}\n}",
        "supaSelectWhere" => "Supa.rest(_activity, \"GET\", {0}, {1}, {2}, \"\") { _ok, _code, _response ->\nif (_ok) {\n{S1}\n} else {\n{S2}\n}\n}",
        "supaInsert" => "Supa.rest(_activity, \"POST\", {1}, \"\", \"\", AppUtil.toJson({0})) { _ok, _code, _response ->\nif (_ok) {\n{S1}\n} else {\n{S2}\n}\n}",
        "supaUpdate" => "Supa.rest(_activity, \"PATCH\", {0}, {1}, {2}, AppUtil.toJson({3})) { _ok, _code, _response ->\nif (_ok) {\n{S1}\n} else {\n{S2}\n}\n}",
        "supaDelete" => "Supa.rest(_activity, \"DELETE\", {0}, {1}, {2}, \"\") { _ok, _code, _response ->\nif (_ok) {\n{S1}\n} else {\n{S2}\n}\n}",
        "supaError" => "Supa.error(_response)",
        "webRequest" => "AppUtil.web(_activity, {0}, {1}, {2}, {3}) { _ok, _code, _response ->\nif (_ok) {\n{S1}\n} else {\n{S2}\n}\n}",
        "webSetHeader" => "{0}.headers[{1}] = {2}",
        "webClearHeaders" => "{0}.headers.clear()",
        "webResponse" => "_response",
        "webStatus" => "_code",
        "mapFromJson" => "{0} = AppUtil.jsonToMap({1})",
        "listMapFromJson" => "{0} = AppUtil.jsonToListMap({1})",
        "mapToJson" | "listMapToJson" => "AppUtil.toJson({0})",
        // More Block
        "moreBlock" => "_{0}()",
        "paramStr" | "paramNum" | "paramBool" => "_p_{0}",
        "toStringFixed" => "AppUtil.fixed({0}, {1})",
        "stringStartsWith" => "{0}.startsWith({1})",
        "stringEndsWith" => "{0}.endsWith({1})",
        "isNumber" => "AppUtil.isNumber({0})",
        "reverseList" => "{0}.reverse()",
        "sortListInt" => "{0}.sort()",
        "sortListStr" => "{0}.sort()",
        "sumList" => "{0}.sum()",
        "splitToList" => "{2}.clear()\n{2}.addAll(AppUtil.split({0}, {1}))",
        "joinList" => "{0}.joinToString({1})",
        "hideKeyboard" => "AppUtil.hideKeyboard(_activity)",
        "openLink" => "AppUtil.openLink(_activity, {0})",
        "shareText" => "AppUtil.share(_activity, {0})",
        _ => return None,
    })
}

/// Swift for a block (inside the screen's model class).
pub fn swift_tpl(op: &str) -> Option<&'static str> {
    Some(match op {
        // Variable
        "setVarBoolean" | "setVarInt" | "setVarString" => "{0} = {1}",
        "increaseInt" => "{0} += 1",
        "decreaseInt" => "{0} -= 1",
        "mapCreateNew" => "{0} = [:]",
        "mapPut" => "{0}[{1}] = {2}",
        "mapGet" => "\"\\({0}[{1}] ?? \"\")\"",
        "mapContainKey" => "({0}[{1}] != nil)",
        "mapRemoveKey" => "{0}.removeValue(forKey: {1})",
        "mapSize" => "Double({0}.count)",
        "mapClear" => "{0}.removeAll()",
        "mapIsEmpty" => "{0}.isEmpty",
        "getVar" => "{0}",
        "eventPosition" => "_position",
        "eventChecked" => "_isChecked",
        "eventText" => "_charSeq",
        "eventProgress" => "_progressValue",
        // List
        "addListInt" | "addListStr" | "addMapToList" => "{1}.append({0})",
        "insertListInt" | "insertListStr" => "{2}.insert({0}, at: Int({1}))",
        "getAtListInt" | "getAtListStr" => "{1}[Int({0})]",
        "indexListInt" | "indexListStr" => "Double({1}.firstIndex(of: {0}) ?? -1)",
        "containListInt" | "containListStr" => "{0}.contains({1})",
        "getMapInList" => "{2} = {1}[Int({0})]",
        "getAtListMap" => "\"\\({2}[Int({0})][{1}] ?? \"\")\"",
        "lengthList" => "Double({0}.count)",
        "deleteList" => "{1}.remove(at: Int({0}))",
        "clearList" => "{0}.removeAll()",
        // Control
        "repeat" => "for _ in 0..<Swift.max(0, Int({0})) {\n{S1}\n}",
        "forever" => "while true {\n{S1}\n}",
        "whileLoop" => "while {0} {\n{S1}\n}",
        "if" => "if {0} {\n{S1}\n}",
        "ifElse" => "if {0} {\n{S1}\n}\nelse {\n{S2}\n}",
        "break" => "break",
        // Operator
        "true" => "true",
        "false" => "false",
        // Double(...) so two plain numbers aren't ambiguous to Swift
        "<" => "Double({0}) < Double({1})",
        "=" => "Double({0}) == Double({1})",
        ">" => "Double({0}) > Double({1})",
        "&&" => "{0} && {1}",
        "||" => "{0} || {1}",
        "not" => "!{0}",
        "+" => "{0} + {1}",
        "-" => "{0} - {1}",
        "*" => "{0} * {1}",
        "/" => "{0} / {1}",
        "%" => "Double({0}).truncatingRemainder(dividingBy: {1})",
        "random" => "Helpers.random(Int({0}), Int({1}))",
        "stringLength" => "Double({0}.utf16.count)",
        "stringJoin" => "{0} + {1}",
        "stringIndex" => "Helpers.indexOf({0}, in: {1})",
        "stringLastIndex" => "Helpers.lastIndexOf({0}, in: {1})",
        "stringSub" => "Helpers.substring({0}, Int({1}), Int({2}))",
        "stringEquals" => "{0} == {1}",
        "stringContains" => "Helpers.contains({0}, {1})",
        "stringReplace" => "{0}.replacingOccurrences(of: {1}, with: {2})",
        "trim" => "{0}.trimmingCharacters(in: .whitespacesAndNewlines)",
        "toUpperCase" => "{0}.uppercased()",
        "toLowerCase" => "{0}.lowercased()",
        "toNumber" => "Helpers.number({0})",
        "toString" => "Helpers.wholeText({0})",
        "toStringWithDecimal" => "Helpers.decimalText({0})",
        // Math
        "mathGetDip" => "Helpers.dip(Int({0}))",
        "mathGetDisplayWidth" => "Helpers.screenWidth",
        "mathGetDisplayHeight" => "Helpers.screenHeight",
        "mathPi" => "Double.pi",
        "mathE" => "M_E",
        "mathPow" => "pow({0}, {1})",
        "mathMin" => "Swift.min({0}, {1})",
        "mathMax" => "Swift.max({0}, {1})",
        "mathSqrt" => "sqrt({0})",
        "mathAbs" => "abs({0})",
        // Java's Math.round: halves go up
        "mathRound" => "floor({0} + 0.5)",
        "mathCeil" => "ceil({0})",
        "mathFloor" => "floor({0})",
        "mathSin" => "sin({0})",
        "mathCos" => "cos({0})",
        "mathTan" => "tan({0})",
        "mathLog" => "log({0})",
        "mathLog10" => "log10({0})",
        "mathToRadian" => "({0}) * Double.pi / 180",
        "mathToDegree" => "({0}) * 180 / Double.pi",
        // File (saved on the phone with UserDefaults)
        "fileGetData" => "{0}.get({1})",
        "fileSetData" => "{0}.set({1}, {2})",
        "fileRemoveData" => "{0}.remove({1})",
        "fileContainsData" => "{0}.contains({1})",
        // View: each widget is a WState the SwiftUI view draws
        "setEnable" => "{0}.enabled = {1}",
        "getEnable" => "{0}.enabled",
        "setVisible" => "{0}.visibility = {1}",
        "setBgColor" => "{0}.bg = {1}",
        "setAlpha" => "{0}.alpha = {1}",
        "setRadius" => "{0}.radius = {1}",
        "setElevation" => "{0}.elevation = {1}",
        "setBorder" => "{0}.border = {1}\n{0}.borderColor = {2}",
        "setText" => "{0}.text = {1}",
        "getText" => "{0}.text",
        "setTextColor" => "{0}.textColor = {1}",
        "setTextSize" => "{0}.textSize = {1}",
        "setFont" => "{0}.font = {1}",
        "setVideo" | "setVideoFrom" => "{0}_video.load({1})",
        "videoPlay" => "{0}_video.play()",
        "videoPause" => "{0}_video.pause()",
        "videoSeek" => "{0}_video.seek({1})",
        "videoLoop" => "{0}_video.setLooping({1})",
        "videoIsPlaying" => "{0}_video.isPlaying",
        "videoPosition" => "{0}_video.position",
        "videoDuration" => "{0}_video.duration",
        "assetText" => "Helpers.assetText({0})",
        "setHint" => "{0}.hint = {1}",
        "setChecked" => "{0}.checked = {1}",
        "getChecked" => "{0}.checked",
        "seekBarSetProgress" => "{0}.progress = Double(Int({1}))",
        "seekBarGetProgress" => "{0}.progress",
        "seekBarSetMax" => "{0}.max = Double(Int({1}))",
        "seekBarGetMax" => "{0}.max",
        "listSetData" => "{0}.items = {1}",
        "webViewLoadUrl" => "{0}.url = {1}",
        "setImage" | "setImageFrom" => "{0}.image = {1}",
        "soundPlay" | "soundPlayFrom" => "{0}.play({1})",
        "cameraPhoto" => "MediaPicker.shared.camera(video: false) { [weak self] _ok, _filePath in\nguard let self else { return }\n_ = self\nif _ok {\n{S1}\n} else {\n{S2}\n}\n}",
        "cameraVideo" => "MediaPicker.shared.camera(video: true) { [weak self] _ok, _filePath in\nguard let self else { return }\n_ = self\nif _ok {\n{S1}\n} else {\n{S2}\n}\n}",
        "pickMedia" => "MediaPicker.shared.pick({0}) { [weak self] _ok, _filePath in\nguard let self else { return }\n_ = self\nif _ok {\n{S1}\n} else {\n{S2}\n}\n}",
        "mediaPath" => "_filePath",
        "soundPause" => "{0}.pause()",
        "soundResume" => "{0}.resume()",
        "soundStop" => "{0}.stop()",
        "soundLoop" => "{0}.setLooping({1})",
        "soundVolume" => "{0}.setVolume({1})",
        "soundSeek" => "{0}.seek({1})",
        "soundIsPlaying" => "{0}.isPlaying",
        "soundPosition" => "{0}.position",
        "soundDuration" => "{0}.duration",
        "requestFocus" => "// {0}: on iOS the keyboard opens when the field is tapped",
        "performClick" => "performClick(\"{0}\")",
        // Component
        "doToast" => "showToast({0})",
        "setTitle" => "screenTitle = {0}",
        "copyToClipboard" => "UIPasteboard.general.string = {0}",
        "finishActivity" => "finish()",
        "intentSetScreen" => "{0}.screen = {1}",
        "intentPutExtra" => "{0}.extras[{1}] = {2}",
        "intentGetString" => "(extras[{0}] ?? \"\")",
        "startActivity" => "startActivity({0})",
        "openScreen" => "startActivity(Intent(screen: {0}))",
        "timerAfter" => "{0}?.invalidate()\n{0} = Helpers.after({1}) { [weak self] in\nguard let self else { return }\n_ = self\n{S1}\n}",
        "timerEvery" => "{0}?.invalidate()\n{0} = Helpers.every(first: {1}, period: {2}) { [weak self] in\nguard let self else { return }\n_ = self\n{S1}\n}",
        "timerCancel" => "{0}?.invalidate()",
        "dialogSetTitle" => "{0}.title = {1}",
        "dialogSetMessage" => "{0}.message = {1}",
        "dialogOkButton" => "{0}.okLabel = {1}\n{0}.ok = { [weak self] in\nguard let self else { return }\n_ = self\n{S1}\n}",
        "dialogCancelButton" => "{0}.cancelLabel = {1}\n{0}.cancel = { [weak self] in\nguard let self else { return }\n_ = self\n{S1}\n}",
        "dialogShow" => "{0}.shown = true",
        // iPhones have no vibration length: a short tap is the closest
        "vibratorAction" => "Helpers.vibrate() // {0}",
        "calendarGetNow" => "{0} = Date()",
        "calendarFormat" => "Helpers.format({0}, {1})",
        "calendarGetTime" => "({0}.timeIntervalSince1970 * 1000)",
        "supaSignUp" => "Supa.auth(\"signup\", {0}, {1}) { [weak self] _ok, _code, _response in\nguard let self else { return }\n_ = self\nif _ok {\n{S1}\n} else {\n{S2}\n}\n}",
        "supaSignIn" => "Supa.auth(\"token?grant_type=password\", {0}, {1}) { [weak self] _ok, _code, _response in\nguard let self else { return }\n_ = self\nif _ok {\n{S1}\n} else {\n{S2}\n}\n}",
        "supaSignOut" => "Supa.signOut()",
        "supaSignedIn" => "Supa.signedIn",
        "supaUserEmail" => "Supa.email",
        "supaUserId" => "Supa.userId",
        "supaSelect" => "Supa.rest(\"GET\", {0}, \"\", \"\", \"\") { [weak self] _ok, _code, _response in\nguard let self else { return }\n_ = self\nif _ok {\n{S1}\n} else {\n{S2}\n}\n}",
        "supaSelectWhere" => "Supa.rest(\"GET\", {0}, {1}, {2}, \"\") { [weak self] _ok, _code, _response in\nguard let self else { return }\n_ = self\nif _ok {\n{S1}\n} else {\n{S2}\n}\n}",
        "supaInsert" => "Supa.rest(\"POST\", {1}, \"\", \"\", Helpers.toJson({0})) { [weak self] _ok, _code, _response in\nguard let self else { return }\n_ = self\nif _ok {\n{S1}\n} else {\n{S2}\n}\n}",
        "supaUpdate" => "Supa.rest(\"PATCH\", {0}, {1}, {2}, Helpers.toJson({3})) { [weak self] _ok, _code, _response in\nguard let self else { return }\n_ = self\nif _ok {\n{S1}\n} else {\n{S2}\n}\n}",
        "supaDelete" => "Supa.rest(\"DELETE\", {0}, {1}, {2}, \"\") { [weak self] _ok, _code, _response in\nguard let self else { return }\n_ = self\nif _ok {\n{S1}\n} else {\n{S2}\n}\n}",
        "supaError" => "Supa.error(_response)",
        "webRequest" => "Helpers.web({0}, {1}, {2}, {3}) { [weak self] _ok, _code, _response in\nguard let self else { return }\n_ = self\nif _ok {\n{S1}\n} else {\n{S2}\n}\n}",
        "webSetHeader" => "{0}.headers[{1}] = {2}",
        "webClearHeaders" => "{0}.headers.removeAll()",
        "webResponse" => "_response",
        "webStatus" => "_code",
        "mapFromJson" => "{0} = Helpers.jsonToMap({1})",
        "listMapFromJson" => "{0} = Helpers.jsonToListMap({1})",
        "mapToJson" | "listMapToJson" => "Helpers.toJson({0})",
        // More Block
        "moreBlock" => "_{0}()",
        "paramStr" | "paramNum" | "paramBool" => "_p_{0}",
        "toStringFixed" => "Helpers.fixed({0}, {1})",
        "stringStartsWith" => "Helpers.starts({0}, {1})",
        "stringEndsWith" => "Helpers.ends({0}, {1})",
        "isNumber" => "Helpers.isNumber({0})",
        "reverseList" => "{0}.reverse()",
        "sortListInt" => "{0}.sort()",
        "sortListStr" => "{0}.sort { $0.utf16.lexicographicallyPrecedes($1.utf16) }",
        "sumList" => "{0}.reduce(0, +)",
        "splitToList" => "{2} = Helpers.split({0}, {1})",
        "joinList" => "{0}.joined(separator: {1})",
        "hideKeyboard" => "Helpers.hideKeyboard()",
        "openLink" => "Helpers.openLink({0})",
        "shareText" => "Helpers.share({0})",
        _ => return None,
    })
}

fn has_event(s: &Screen, target: &str, name: &str) -> bool {
    s.events.iter().any(|e| e.target == target && e.name == name)
}

fn event<'a>(s: &'a Screen, target: &str, name: &str) -> Option<&'a Event> {
    s.events.iter().find(|e| e.target == target && e.name == name)
}

// ---------------------------------------------------------------- Kotlin

fn kotlin_listener(w: &Widget, e: &Event, body: &str) -> String {
    let id = &w.id;
    match e.name.as_str() {
        "onClick" => format!("{id}.setOnClickListener {{\n{body}\n}}"),
        "onLongClick" => format!("{id}.setOnLongClickListener {{\n{body}\ntrue\n}}"),
        "onCheckedChange" => format!("{id}.setOnCheckedChangeListener {{ _, _isChecked ->\n{body}\n}}"),
        "onTextChanged" => format!("{id}.addTextChangedListener(object : TextWatcher {{\noverride fun onTextChanged(_param1: CharSequence, _param2: Int, _param3: Int, _param4: Int) {{\nval _charSeq = _param1.toString()\n{body}\n}}\noverride fun beforeTextChanged(_param1: CharSequence, _param2: Int, _param3: Int, _param4: Int) {{\n}}\noverride fun afterTextChanged(_param1: Editable) {{\n}}\n}})"),
        "onProgressChanged" => format!("{id}.setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {{\noverride fun onProgressChanged(_param1: SeekBar, _param2: Int, _param3: Boolean) {{\nval _progressValue = _param2.toDouble()\n{body}\n}}\noverride fun onStartTrackingTouch(_param1: SeekBar) {{\n}}\noverride fun onStopTrackingTouch(_param1: SeekBar) {{\n}}\n}})"),
        "onItemClicked" => format!("{id}.setOnItemClickListener {{ _, _, _param3, _ ->\nval _position = _param3.toDouble()\n{body}\n}}"),
        "onItemSelected" => format!("{id}.onItemSelectedListener = object : AdapterView.OnItemSelectedListener {{\noverride fun onItemSelected(_param1: AdapterView<*>?, _param2: View?, _param3: Int, _param4: Long) {{\nval _position = _param3.toDouble()\n{body}\n}}\noverride fun onNothingSelected(_param1: AdapterView<*>?) {{\n}}\n}}"),
        "onCompletion" => format!("{id}_video.onEnd = {{\n{body}\n}}"),
        _ => format!("// {}", e.name),
    }
}

/// The blocks that open the camera or the phone's files and wait for them.
pub(crate) const MEDIA_OPS: &[&str] = &["cameraPhoto", "cameraVideo", "pickMedia"];

/// Whether any of the screen's blocks is one of `ops`.
pub(crate) fn uses_ops(s: &Screen, ops: &[&str]) -> bool {
    fn any(bs: &[Block], ops: &[&str]) -> bool {
        bs.iter().any(|b| {
            ops.contains(&b.op.as_str())
                || b.args.iter().any(|a| matches!(a, Arg::Block(x) if any(std::slice::from_ref(&**x), ops)))
                || any(&b.sub1, ops)
                || any(&b.sub2, ops)
        })
    }
    s.events.iter().any(|e| any(&e.blocks, ops))
}

/// One screen's Activity in Kotlin, with its layout file beside it.
pub fn kotlin(p: &Project, s: &Screen) -> String {
    kotlin_with(p, s, false)
}

/// `trace`: a debug build that logs each event, block and variable change.
pub fn kotlin_with(p: &Project, s: &Screen, trace: bool) -> String {
    let mut g = Gen::new(s, Lang::Kotlin);
    g.trace = trace;
    let act = s.activity_name();
    let mut src = format!("package {}\n\n", p.package);
    for imp in ["android.app.*", "android.content.*", "android.graphics.*", "android.net.*", "android.os.*", "android.text.*", "android.util.*", "android.view.*", "android.webkit.*", "android.widget.*", "java.text.*", "java.util.*"] {
        src.push_str(&format!("import {imp}\n"));
    }
    src.push_str(&format!("\nclass {act} : Activity() {{\n\n"));
    // the screen, also from inside timers and other objects where `this` is them
    src.push_str("private val _activity: Activity get() = this\n\n");
    if s.components.iter().any(|c| c.kind == CompKind::Timer) {
        src.push_str("private val _timer = Timer()\n\n");
    }
    for v in &s.vars {
        src.push_str(&match v.ty {
            VarType::Bool => format!("private var {} = false\n", v.name),
            VarType::Num => format!("private var {} = 0.0\n", v.name),
            VarType::Str => format!("private var {} = \"\"\n", v.name),
            VarType::Map => format!("private var {} = HashMap<String, Any>()\n", v.name),
        });
    }
    for l in &s.lists {
        src.push_str(&match l.ty {
            ListType::Num => format!("private var {} = ArrayList<Double>()\n", l.name),
            ListType::Str => format!("private var {} = ArrayList<String>()\n", l.name),
            ListType::Map => format!("private var {} = ArrayList<HashMap<String, Any>>()\n", l.name),
        });
    }
    if !s.vars.is_empty() || !s.lists.is_empty() {
        src.push('\n');
    }
    for w in &s.widgets {
        src.push_str(&format!("private lateinit var {}: {}\n", w.id, w.kind.android_class()));
    }
    for c in &s.components {
        src.push_str(&match c.kind {
            CompKind::Intent => format!("private var {} = Intent()\n", c.name),
            CompKind::SharedPref => format!("private lateinit var {}: SharedPreferences\n", c.name),
            CompKind::Timer => format!("private var {}: TimerTask? = null\n", c.name),
            CompKind::Dialog => format!("private lateinit var {}: AlertDialog.Builder\n", c.name),
            CompKind::Vibrator => format!("private lateinit var {}: Vibrator\n", c.name),
            CompKind::Calendar => format!("private var {}: Calendar = Calendar.getInstance()\n", c.name),
            CompKind::Web => format!("private val {} = WebClient()\n", c.name),
            CompKind::Sound => format!("private val {} = SoundPlayer()\n", c.name),
            CompKind::Camera => format!("private val {} = \"camera\"\n", c.name),
        });
    }
    for w in s.widgets.iter().filter(|w| w.kind == WKind::VideoView) {
        src.push_str(&format!("private lateinit var {}_video: VideoControl\n", w.id));
    }
    src.push_str("\noverride fun onCreate(savedInstanceState: Bundle?) {\nsuper.onCreate(savedInstanceState)\n");
    let sheet = p.sheet_of(s);
    if sheet == Sheet::No {
        src.push_str(&kotlin_app_bar(&s.look()));
        src.push_str(&format!("setContentView(R.layout.{})\n", s.name));
    } else {
        // a bottom sheet: a window on the lower part of the phone, over the
        // screen that opened it; a tap above it closes it
        src.push_str(&format!("setContentView(R.layout.{})\nfindViewById<View>(android.R.id.content).setPadding(0, ({SHEET_TOP} * resources.displayMetrics.density).toInt(), 0, 0)\n", s.name));
        src.push_str(&format!("window.setLayout(ViewGroup.LayoutParams.MATCH_PARENT, (resources.displayMetrics.heightPixels * {}).toInt())\nwindow.setGravity(Gravity.BOTTOM)\nsetFinishOnTouchOutside(true)\n", sheet.height()));
    }
    src.push_str("initialize()\ninitializeLogic()\n}\n\n");
    if sheet != Sheet::No {
        // drag it down to close it (AppUtil.kt)
        src.push_str("private val sheetDrag by lazy { SheetDrag(this) }\n\noverride fun dispatchTouchEvent(ev: android.view.MotionEvent): Boolean = sheetDrag.dispatch(ev) { super.dispatchTouchEvent(it) }\n\n");
    }
    let barless = s.app_bar.hidden || sheet != Sheet::No;
    let actions: &[BarAction] = if barless { &[] } else { &s.app_bar.actions };
    if !actions.is_empty() {
        // the bar's icon buttons, in the title's colour
        src.push_str("override fun onCreateOptionsMenu(menu: android.view.Menu): Boolean {\n");
        for (i, a) in actions.iter().enumerate() {
            src.push_str(&format!(
                "menu.add(0, {}, {i}, {}).apply {{\nsetIcon(R.drawable.{})\nsetShowAsAction(android.view.MenuItem.SHOW_AS_ACTION_IF_ROOM)\nicon?.mutate()?.setTint(0x{:08X}.toInt())\n}}\n",
                i + 1,
                crate::codegen::quote_for(if a.label.is_empty() { &a.icon } else { &a.label }, Lang::Kotlin),
                android_icon_name(&a.icon),
                s.app_bar.title(false)
            ));
        }
        src.push_str("return true\n}\n\n");
    }
    let up = !barless && !s.app_bar.no_back;
    if up || !actions.is_empty() {
        src.push_str("override fun onOptionsItemSelected(item: android.view.MenuItem): Boolean {\nwhen (item.itemId) {\n");
        if up {
            // the up arrow does what the phone's back does
            src.push_str("android.R.id.home -> onBackPressed()\n");
        }
        for (i, a) in actions.iter().enumerate() {
            let body = event(s, &a.id, "onClick").map(|e| g.event_body(e)).unwrap_or_default();
            src.push_str(&format!("{} -> {{\n{body}\n}}\n", i + 1));
        }
        src.push_str("else -> return super.onOptionsItemSelected(item)\n}\nreturn true\n}\n\n");
    }
    if !barless && s.app_bar.title_color != 0 {
        // the title keeps its colour, setTitle blocks included
        src.push_str(&format!("override fun onTitleChanged(title: CharSequence?, color: Int) {{\nval t = android.text.SpannableString(title ?: \"\")\nt.setSpan(android.text.style.ForegroundColorSpan(0x{:08X}.toInt()), 0, t.length, 0)\nsuper.onTitleChanged(t, color)\n}}\n\n", s.app_bar.title(false)));
    }
    src.push_str("private fun initialize() {\n");
    for w in &s.widgets {
        src.push_str(&format!("{0} = findViewById(R.id.{0})\n", w.id));
    }
    // a password field's eye shows or hides what's typed
    for w in s.widgets.iter().filter(|w| w.kind == WKind::EditText && w.input_kind() == InputKind::Password) {
        src.push_str(&format!("AppUtil.passwordEye({})\n", w.id));
    }
    // corners, outlines and shadows have no plain layout attribute
    for w in &s.widgets {
        if w.kind == WKind::ImageView && !w.image.is_empty() {
            src.push_str(&format!("AppUtil.setPicture({}, {})\n", w.id, crate::codegen::quote_for(&w.image, Lang::Kotlin)));
        }
        if !w.font.is_empty() && w.kind.has_text() {
            src.push_str(&format!("AppUtil.setFont({}, {})\n", w.id, crate::codegen::quote_for(&w.font, Lang::Kotlin)));
        }
        if w.kind == WKind::VideoView {
            src.push_str(&format!("{0}_video = VideoControl({0})\n", w.id));
            if !w.controls {
                src.push_str(&format!("{}_video.setControls(false)\n", w.id));
            }
            if w.looping {
                src.push_str(&format!("{}_video.setLooping(true)\n", w.id));
            }
            if !w.video.is_empty() {
                src.push_str(&format!("{}_video.load({})\n", w.id, crate::codegen::quote_for(&w.video, Lang::Kotlin)));
            }
            if w.autoplay {
                src.push_str(&format!("{}_video.play()\n", w.id));
            }
        }
        if w.radius > 0 {
            src.push_str(&format!("AppUtil.setRadius({}, {}.0)\n", w.id, w.radius));
        }
        if w.border > 0 {
            src.push_str(&format!("AppUtil.setBorder({}, {}.0, 0x{:08X}.toInt())\n", w.id, w.border, w.border_color));
        }
        if w.elevation > 0 {
            src.push_str(&format!("AppUtil.setElevation({}, {}.0)\n", w.id, w.elevation));
        }
    }
    for c in &s.components {
        match c.kind {
            CompKind::SharedPref => src.push_str(&format!("{0} = getSharedPreferences(\"{0}\", Activity.MODE_PRIVATE)\n", c.name)),
            CompKind::Dialog => src.push_str(&format!("{} = AlertDialog.Builder(this)\n", c.name)),
            CompKind::Vibrator => src.push_str(&format!("{} = getSystemService(Context.VIBRATOR_SERVICE) as Vibrator\n", c.name)),
            _ => {}
        }
    }
    for e in &s.events {
        if e.target.is_empty() || e.target == "moreBlock" {
            continue;
        }
        if let Some(w) = s.widget(&e.target) {
            let body = g.event_body(e);
            src.push('\n');
            src.push_str(&kotlin_listener(w, e, &body));
            src.push('\n');
        }
    }
    src.push_str("}\n\n");
    if uses_ops(s, MEDIA_OPS) {
        src.push_str("override fun onActivityResult(requestCode: Int, resultCode: Int, data: android.content.Intent?) {\nif (!AppMedia.onResult(requestCode, resultCode, data)) super.onActivityResult(requestCode, resultCode, data)\n}\n\n");
    }
    let players: Vec<&Component> = s.components.iter().filter(|c| c.kind == CompKind::Sound).collect();
    if !players.is_empty() {
        src.push_str("override fun onDestroy() {\nsuper.onDestroy()\n");
        for c in players {
            src.push_str(&format!("{}.stop()\n", c.name));
        }
        src.push_str("}\n\n");
    }
    src.push_str("private fun initializeLogic() {\n");
    if !s.title.is_empty() {
        src.push_str(&format!("title = {}\n", crate::codegen::quote_for(&s.title, Lang::Kotlin)));
    }
    if let Some(e) = event(s, "", "onCreate") {
        src.push_str(&g.event_body(e));
        src.push('\n');
    }
    src.push_str("}\n");
    if let Some(e) = event(s, "", "onBackPressed") {
        src.push_str("\noverride fun onBackPressed() {\n");
        src.push_str(&g.event_body(e));
        src.push_str("\n}\n");
    }
    for e in s.events.iter().filter(|e| e.target == "moreBlock") {
        let params: Vec<String> = e
            .params
            .iter()
            .map(|p| {
                let ty = match p.ty {
                    VarType::Num => "Double",
                    VarType::Bool => "Boolean",
                    _ => "String",
                };
                format!("_p_{}: {ty}", p.name)
            })
            .collect();
        src.push_str(&format!("\nprivate fun _{}({}) {{\n", e.name, params.join(", ")));
        g.params = e.params.iter().map(|p| p.name.clone()).collect();
        src.push_str(&g.event_body(e));
        g.params.clear();
        src.push_str("\n}\n");
    }
    src.push_str("\n}\n");
    indent(&src, 0)
}

// ---------------------------------------------------------------- Swift

fn swift_color(c: u32) -> String {
    format!("0x{:08X}", c)
}

fn swift_wstate(w: &Widget) -> String {
    let mut parts = vec![];
    if !w.text.is_empty() {
        parts.push(format!("text: {}", quote(&w.text)));
    }
    if !w.hint.is_empty() && w.kind == WKind::EditText {
        parts.push(format!("hint: {}", quote(&w.hint)));
    }
    if !w.enabled {
        parts.push("enabled: false".into());
    }
    if !w.visible {
        parts.push("visibility: 2".into());
    }
    if w.checked {
        parts.push("checked: true".into());
    }
    if w.progress != 0 {
        parts.push(format!("progress: {}", w.progress));
    }
    if w.max != 100 {
        parts.push(format!("max: {}", w.max));
    }
    parts.push(format!("bg: {}", swift_color(w.bg)));
    parts.push(format!("textColor: {}", swift_color(w.text_color)));
    parts.push(format!("textSize: {}", w.text_size));
    // Swift wants these in the order WState declares them
    if w.radius > 0 {
        parts.push(format!("radius: {}", w.radius));
    }
    if w.elevation > 0 {
        parts.push(format!("elevation: {}", w.elevation));
    }
    if w.border > 0 {
        parts.push(format!("border: {}", w.border));
        parts.push(format!("borderColor: {}", swift_color(w.border_color)));
    }
    if !w.image.is_empty() {
        parts.push(format!("image: {}", quote(&w.image)));
    }
    if !w.font.is_empty() {
        parts.push(format!("font: {}", quote(&w.font)));
    }
    format!("WState({})", parts.join(", "))
}

fn size_rule(s: Size) -> String {
    match s {
        Size::Wrap => ".wrap".into(),
        Size::Match => ".fill".into(),
        Size::Px(n) => format!(".fixed({n})"),
    }
}

fn swift_children(s: &Screen, parent: &str, out: &mut String) {
    for i in s.children(parent) {
        swift_widget(s, &s.widgets[i], out);
    }
}

fn swift_font(w: &Widget) -> String {
    format!(".font(Helpers.font(m.{0}.font, size: m.{0}.textSize, bold: {1}))\n.foregroundColor(Color(argb: m.{0}.textColor))\n", w.id, w.bold)
}

fn swift_widget(s: &Screen, w: &Widget, out: &mut String) {
    let id = &w.id;
    let h_align = match w.gravity {
        Gravity::Center => ".center",
        Gravity::Right => ".trailing",
        _ => ".leading",
    };
    let v_align = match w.vgravity {
        crate::model::VGravity::Center => ".center",
        crate::model::VGravity::Bottom => ".bottom",
        _ => ".top",
    };
    // an Android button centres its text unless its gravity says otherwise
    let (h_align, v_align) = if w.kind == WKind::Button {
        (if w.gravity == Gravity::None { ".center" } else { h_align }, if w.vgravity == crate::model::VGravity::None { ".center" } else { v_align })
    } else {
        (h_align, v_align)
    };
    let box_align = match (h_align, v_align) {
        (".center", ".center") => ".center".to_string(),
        (".center", v) => v.to_string(),
        (h, ".center") => h.to_string(),
        (h, v) => format!("{v}{}{}", h[1..2].to_uppercase(), &h[2..]),
    };
    out.push_str(&format!("if m.{id}.visibility != 2 {{\n"));
    match w.kind {
        WKind::LinearV | WKind::LinearH if has_weights(s, id) => {
            let vertical = w.kind == WKind::LinearV;
            let cross = match if vertical { h_align } else { v_align } {
                ".center" => "0.5",
                ".trailing" | ".bottom" => "1",
                _ => "0",
            };
            out.push_str(&format!("Linear(vertical: {vertical}, cross: {cross}) {{\n"));
            swift_children(s, id, out);
            out.push_str("}\n");
        }
        WKind::LinearV => {
            out.push_str(&format!("VStack(alignment: {h_align}, spacing: 0) {{\n"));
            swift_children(s, id, out);
            out.push_str("}\n");
        }
        WKind::LinearH => {
            out.push_str(&format!("HStack(alignment: {v_align}, spacing: 0) {{\n"));
            swift_children(s, id, out);
            out.push_str("}\n");
        }
        WKind::ScrollV => {
            out.push_str(&format!("ScrollView {{\nVStack(alignment: {h_align}, spacing: 0) {{\n"));
            swift_children(s, id, out);
            out.push_str("}\n}\n");
        }
        WKind::ScrollH => {
            out.push_str(&format!("ScrollView(.horizontal) {{\nHStack(alignment: {v_align}, spacing: 0) {{\n"));
            swift_children(s, id, out);
            out.push_str("}\n}\n");
        }
        WKind::TextView => {
            out.push_str(&format!("Text(m.{id}.text)\n"));
            out.push_str(&swift_font(w));
        }
        WKind::Button => {
            let action = if has_event(s, id, "onClick") { format!("m.{id}_onClick()\n") } else { String::new() };
            out.push_str(&format!("Button {{\n{action}}} label: {{\nText(m.{id}.text)\n"));
            out.push_str(&swift_font(w));
            // the label takes the button's whole size, so the text sits where
            // its gravity says and a tap anywhere on the button counts
            let (bw, bh) = weighted_size(s, w);
            let fill = |z: Size| if z == Size::Wrap { "nil" } else { ".infinity" };
            out.push_str(&format!(".frame(maxWidth: {}, maxHeight: {}, alignment: {box_align})\n.contentShape(Rectangle())\n", fill(bw), fill(bh)));
            // a button with its own colour is just that colour, as on Android
            out.push_str(if (w.bg >> 24) == 0 { "}\n.buttonStyle(.bordered)\n" } else { "}\n.buttonStyle(.plain)\n" });
        }
        WKind::EditText => {
            let binding = if has_event(s, id, "onTextChanged") {
                format!("Binding(get: {{ m.{id}.text }}, set: {{ m.{id}.text = $0; m.{id}_onTextChanged($0) }})")
            } else {
                format!("$m.{id}.text")
            };
            let input = w.input_kind();
            match input {
                InputKind::Password => out.push_str(&format!("PasswordField(hint: m.{id}.hint, text: {binding})\n")),
                InputKind::Multiline => out.push_str(&format!("TextField(m.{id}.hint, text: {binding}, axis: .vertical)\n.lineLimit(2...8)\n")),
                _ => out.push_str(&format!("TextField(m.{id}.hint, text: {binding})\n")),
            }
            out.push_str(&swift_font(w));
            if input != InputKind::Password {
                out.push_str(".textFieldStyle(.roundedBorder)\n");
            }
            // the keyboard it brings up; numbers and phone numbers keep out letters
            let keys = match input {
                InputKind::Email => ".keyboardType(.emailAddress)\n.textInputAutocapitalization(.never)\n.autocorrectionDisabled()\n",
                InputKind::Number => ".keyboardType(.numbersAndPunctuation)\n",
                InputKind::Decimal => ".keyboardType(.decimalPad)\n",
                InputKind::Phone => ".keyboardType(.phonePad)\n",
                _ => "",
            };
            out.push_str(keys);
            let allowed = match input {
                InputKind::Number => Some("0123456789-"),
                InputKind::Decimal => Some("0123456789-.,"),
                InputKind::Phone => Some("0123456789+-() #*"),
                _ => None,
            };
            if let Some(chars) = allowed {
                out.push_str(&format!(".onChange(of: m.{id}.text) {{ _, v in\nlet f = v.filter {{ \"{chars}\".contains($0) }}\nif f != v {{ m.{id}.text = f }}\n}}\n"));
            }
        }
        WKind::ImageView => {
            let natural = w.width == Size::Wrap && w.height == Size::Wrap;
            // a box with both sides set: the picture fits in it, centred (Android's fitCenter)
            let fill = w.width != Size::Wrap && w.height != Size::Wrap;
            out.push_str(&format!("PictureView(name: m.{id}.image, crop: {}, natural: {natural}, fill: {fill})\n", w.crop));
        }
        WKind::WebView => {
            out.push_str(&format!("WebPage(url: m.{id}.url)\n"));
        }
        WKind::VideoView => {
            out.push_str(&format!("VideoBox(control: m.{id}_video, controls: {})\n", w.controls));
            if w.width == Size::Wrap || w.height == Size::Wrap {
                out.push_str(".aspectRatio(16.0 / 9.0, contentMode: .fit)\n");
            }
        }
        WKind::ProgressBar => {
            out.push_str(&format!("ProgressView(value: Swift.min(m.{id}.progress, Swift.max(m.{id}.max, 1)), total: Swift.max(m.{id}.max, 1))\n"));
        }
        WKind::SeekBar => {
            let binding = if has_event(s, id, "onProgressChanged") {
                format!("Binding(get: {{ m.{id}.progress }}, set: {{ m.{id}.progress = $0; m.{id}_onProgressChanged($0) }})")
            } else {
                format!("$m.{id}.progress")
            };
            out.push_str(&format!("Slider(value: {binding}, in: 0...Swift.max(m.{id}.max, 1), step: 1)\n"));
        }
        WKind::ListView => {
            out.push_str(&format!("VStack(alignment: .leading, spacing: 0) {{\nForEach(Array(m.{id}.items.enumerated()), id: \\.offset) {{ i, item in\nText(item)\n.padding(10)\n.frame(maxWidth: .infinity, alignment: .leading)\n.contentShape(Rectangle())\n"));
            if has_event(s, id, "onItemClicked") {
                out.push_str(&format!(".onTapGesture {{ m.{id}_onItemClicked(Double(i)) }}\n"));
            }
            out.push_str("Divider()\n}\n}\n");
        }
        WKind::Spinner => {
            let call = if has_event(s, id, "onItemSelected") { format!("; m.{id}_onItemSelected(Double($0))") } else { String::new() };
            out.push_str(&format!("Picker(\"\", selection: Binding(get: {{ m.{id}.selected }}, set: {{ m.{id}.selected = $0{call} }})) {{\nForEach(Array(m.{id}.items.enumerated()), id: \\.offset) {{ i, item in\nText(item).tag(i)\n}}\n}}\n.pickerStyle(.menu)\n"));
        }
        WKind::CheckBox => {
            // a button drawing the box from the model itself: a ToggleStyle
            // with a hand-made binding could draw the box a tap behind
            let call = if has_event(s, id, "onCheckedChange") { format!("; m.{id}_onCheckedChange(v)") } else { String::new() };
            out.push_str(&format!("Button {{ let v = !m.{id}.checked; m.{id}.checked = v{call} }} label: {{\nHStack(spacing: 8) {{\nImage(systemName: m.{id}.checked ? \"checkmark.square.fill\" : \"square\")\nText(m.{id}.text)\n}}\n}}\n.buttonStyle(.plain)\n"));
            out.push_str(&swift_font(w));
            out.push_str(&format!(".accessibilityAddTraits(m.{id}.checked ? .isSelected : [])\n"));
        }
        WKind::Switch => {
            let binding = if has_event(s, id, "onCheckedChange") {
                format!("Binding(get: {{ m.{id}.checked }}, set: {{ m.{id}.checked = $0; m.{id}_onCheckedChange($0) }})")
            } else {
                format!("$m.{id}.checked")
            };
            out.push_str(&format!("Toggle(m.{id}.text, isOn: {binding})\n"));
            out.push_str(&swift_font(w));
            out.push_str(".toggleStyle(.switch)\n");
        }
        WKind::CalendarView => {
            out.push_str(&format!("DatePicker(\"\", selection: $m.{id}.date, displayedComponents: .date)\n.datePickerStyle(.graphical)\n.labelsHidden()\n"));
        }
    }
    if w.kind != WKind::Button && has_event(s, id, "onClick") {
        out.push_str(&format!(".contentShape(Rectangle())\n.onTapGesture {{ m.{id}_onClick() }}\n"));
    }
    if has_event(s, id, "onLongClick") {
        out.push_str(&format!(".simultaneousGesture(LongPressGesture().onEnded {{ _ in m.{id}_onLongClick() }})\n"));
    }
    // a weight fills along the parent's direction; equal fills share the
    // room equally in a SwiftUI stack
    let (width, height) = weighted_size(s, w);
    out.push_str(&format!(".widgetBox(m.{id}, width: {}, height: {}, padding: {}, margin: {}, align: {box_align})\n", size_rule(width), size_rule(height), w.padding, w.margin));
    if has_weights(s, &w.parent) {
        out.push_str(&format!(".linearItem(weight: {:?}, margin: {})\n", w.weight.max(0.0) as f64, w.margin));
    }
    out.push_str("}\n");
}

/// A weight fills along the parent's direction (the parent shares the room).
fn weighted_size(s: &Screen, w: &Widget) -> (Size, Size) {
    let in_row = s.widgets.iter().find(|p| p.id == w.parent).is_some_and(|p| p.kind.is_horizontal());
    match (w.weight > 0.0, in_row) {
        (true, true) => (Size::Match, w.height),
        (true, false) => (w.width, Size::Match),
        _ => (w.width, w.height),
    }
}

/// A row or column with a weighted child: SwiftUI stacks share room equally,
/// so these use the `Linear` layout, which shares it by weight as Android does.
fn has_weights(s: &Screen, id: &str) -> bool {
    s.widgets.iter().find(|p| p.id == id).is_some_and(|p| matches!(p.kind, WKind::LinearV | WKind::LinearH))
        && s.widgets.iter().any(|c| c.parent == id && c.weight > 0.0)
}

/// The Swift type name of a screen: "main" → "Main" (MainScreen, MainModel).
pub fn swift_name(s: &Screen) -> String {
    let a = s.activity_name();
    a.strip_suffix("Activity").filter(|b| !b.is_empty()).map(str::to_string).unwrap_or(a)
}

/// One screen in SwiftUI: a model (the blocks) and a view (the layout).
pub fn swift_screen(p: &Project, s: &Screen) -> String {
    swift_screen_with(p, s, false)
}

/// `trace`: a debug build that prints each event, block and variable change.
pub fn swift_screen_with(p: &Project, s: &Screen, trace: bool) -> String {
    let mut g = Gen::new(s, Lang::Swift);
    g.trace = trace;
    let act = swift_name(s);
    let model = format!("{act}Model");
    let mut src = String::from("import SwiftUI\n\n");
    // ----- model
    src.push_str(&format!("final class {model}: ObservableObject {{\nlet router: Router\nlet extras: [String: String]\nprivate var started = false\nprivate var opened = false\nprivate var openedSheet = false\n"));
    let title = if s.title.is_empty() { p.app_name.clone() } else { s.title.clone() };
    src.push_str(&format!("@Published var screenTitle = {}\n@Published var toastText: String? = nil\n\n", quote(&title)));
    for v in &s.vars {
        src.push_str(&match v.ty {
            VarType::Bool => format!("@Published var {} = false\n", v.name),
            VarType::Num => format!("@Published var {}: Double = 0\n", v.name),
            VarType::Str => format!("@Published var {} = \"\"\n", v.name),
            VarType::Map => format!("@Published var {}: [String: Any] = [:]\n", v.name),
        });
    }
    for l in &s.lists {
        src.push_str(&match l.ty {
            ListType::Num => format!("@Published var {}: [Double] = []\n", l.name),
            ListType::Str => format!("@Published var {}: [String] = []\n", l.name),
            ListType::Map => format!("@Published var {}: [[String: Any]] = []\n", l.name),
        });
    }
    for w in &s.widgets {
        src.push_str(&format!("@Published var {} = {}\n", w.id, swift_wstate(w)));
    }
    for c in &s.components {
        src.push_str(&match c.kind {
            CompKind::Intent => format!("var {} = Intent()\n", c.name),
            CompKind::SharedPref => format!("let {0} = Prefs(name: \"{0}\")\n", c.name),
            CompKind::Timer => format!("var {}: Timer? = nil\n", c.name),
            CompKind::Dialog => format!("@Published var {} = DialogState()\n", c.name),
            CompKind::Vibrator => format!("let {} = \"vibrator\"\n", c.name),
            CompKind::Calendar => format!("var {} = Date()\n", c.name),
            CompKind::Web => format!("let {} = WebClient()\n", c.name),
            CompKind::Sound => format!("let {} = SoundPlayer()\n", c.name),
            CompKind::Camera => format!("let {} = \"camera\"\n", c.name),
        });
    }
    let videos: Vec<&Widget> = s.widgets.iter().filter(|w| w.kind == WKind::VideoView).collect();
    for w in &videos {
        src.push_str(&format!("let {}_video = VideoControl()\n", w.id));
    }
    src.push_str("\ninit(router: Router, extras: [String: String]) {\nself.router = router\nself.extras = extras\n}\n\n");
    src.push_str("func start() {\nif started { return }\nstarted = true\n");
    // videos are set up before the screen's blocks run, as on Android
    for w in &videos {
        if has_event(s, &w.id, "onCompletion") {
            src.push_str(&format!("{0}_video.onEnd = {{ [weak self] in self?.{0}_onCompletion() }}\n", w.id));
        }
        if w.looping {
            src.push_str(&format!("{}_video.setLooping(true)\n", w.id));
        }
        if !w.video.is_empty() {
            src.push_str(&format!("{}_video.load({})\n", w.id, quote(&w.video)));
        }
        if w.autoplay {
            src.push_str(&format!("{}_video.play()\n", w.id));
        }
    }
    if let Some(e) = event(s, "", "onCreate") {
        src.push_str(&g.event_body(e));
        src.push('\n');
    }
    src.push_str("}\n\n");
    src.push_str("func showToast(_ text: String) {\ntoastText = text\nDispatchQueue.main.asyncAfter(deadline: .now() + 2) { [weak self] in\nif self?.toastText == text { self?.toastText = nil }\n}\n}\n\n");
    // after startActivity, finish closes this screen (below the new one), as on Android
    if p.sheet_of(s) != Sheet::No {
        // a message showing on the sheet goes on showing on the screen below
        src.push_str("func finish() {\nif let t = toastText { router.showToast(t) }\nrouter.sheet = nil\n}\n\n");
    } else {
        // "open screen" then "close this screen": the opened one stays, this one leaves
        src.push_str("func finish() {\nif openedSheet {\nopenedSheet = false\nif !router.path.isEmpty { router.path.removeLast() }\n} else if opened {\nopened = false\nif router.path.count >= 2 { router.path.remove(at: router.path.count - 2) }\n} else if !router.path.isEmpty {\nrouter.path.removeLast()\n}\n}\n\n");
    }
    src.push_str("func startActivity(_ intent: Intent) {\nlet route = Route(screen: intent.screen, extras: intent.extras)\nif Router.sheets.contains(intent.screen) {\nrouter.sheet = route\nopenedSheet = true\n} else {\nrouter.sheet = nil\nrouter.path.append(route)\nopened = true\n}\nDispatchQueue.main.async { [weak self] in self?.opened = false; self?.openedSheet = false }\n}\n\n");
    src.push_str("func performClick(_ id: String) {\nswitch id {\n");
    for e in s.events.iter().filter(|e| e.name == "onClick" && !e.target.is_empty() && e.target != "moreBlock") {
        src.push_str(&format!("case \"{0}\": {0}_onClick()\n", e.target));
    }
    src.push_str("default: break\n}\n}\n");
    for e in &s.events {
        let params = match e.name.as_str() {
            "onCheckedChange" => "_ _isChecked: Bool",
            "onTextChanged" => "_ _charSeq: String",
            "onProgressChanged" => "_ _progressValue: Double",
            "onItemClicked" | "onItemSelected" => "_ _position: Double",
            _ => "",
        };
        let more_params: String;
        let params = if e.target == "moreBlock" {
            more_params = e
                .params
                .iter()
                .map(|p| {
                    let ty = match p.ty {
                        VarType::Num => "Double",
                        VarType::Bool => "Bool",
                        _ => "String",
                    };
                    format!("_ _p_{}: {ty}", p.name)
                })
                .collect::<Vec<_>>()
                .join(", ");
            more_params.as_str()
        } else {
            params
        };
        let fname = if e.target == "moreBlock" {
            format!("_{}", e.name)
        } else if e.target.is_empty() {
            if e.name == "onCreate" {
                continue;
            }
            e.name.clone()
        } else {
            format!("{}_{}", e.target, e.name)
        };
        src.push_str(&format!("\nfunc {fname}({params}) {{\n"));
        if e.target == "moreBlock" {
            g.params = e.params.iter().map(|p| p.name.clone()).collect();
        }
        src.push_str(&g.event_body(e));
        g.params.clear();
        src.push_str("\n}\n");
    }
    src.push_str("}\n\n");
    // ----- view
    src.push_str(&format!(
        "struct {act}Screen: View {{\n@StateObject private var m: {model}\n\ninit(router: Router, extras: [String: String] = [:]) {{\n_m = StateObject(wrappedValue: {model}(router: router, extras: extras))\n}}\n\nvar body: some View {{\nVStack(alignment: .leading, spacing: 0) {{\n"
    ));
    swift_children(s, "root", &mut src);
    src.push_str("Spacer(minLength: 0)\n}\n.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)\n.navigationTitle(m.screenTitle)\n.navigationBarTitleDisplayMode(.inline)\n.overlay(alignment: .bottom) { ToastView(text: m.toastText) }\n");
    for c in s.components.iter().filter(|c| c.kind == CompKind::Dialog) {
        let d = &c.name;
        src.push_str(&format!(".alert(m.{d}.title, isPresented: $m.{d}.shown) {{\nif let l = m.{d}.okLabel {{ Button(l) {{ m.{d}.ok?() }} }}\nif let l = m.{d}.cancelLabel {{ Button(l, role: .cancel) {{ m.{d}.cancel?() }} }}\n}} message: {{\nText(m.{d}.message)\n}}\n"));
    }
    let sheet = p.sheet_of(s);
    if sheet != Sheet::No {
        src.push_str(&format!(".padding(.top, {SHEET_TOP})\n.presentationDetents([.{}])\n.presentationDragIndicator(.visible)\n.presentationBackground(Color.white)\n", if sheet == Sheet::Half { "medium" } else { "large" }));
    } else {
        src.push_str(&swift_app_bar(s));
    }
    if sheet != Sheet::No {
    } else if s.app_bar.no_back {
        src.push_str(".navigationBarBackButtonHidden(true)\n");
    } else if has_event(s, "", "onBackPressed") {
        src.push_str(".navigationBarBackButtonHidden(true)\n.toolbar {\nToolbarItem(placement: .navigationBarLeading) {\nButton(\"Back\") { m.onBackPressed() }\n}\n}\n");
    }
    let look = s.look();
    let light = match look.color {
        c if c != 0 => if is_light(if look.hidden { c } else { look.bar(true) }) { "false" } else { "true" },
        _ => "nil",
    };
    if sheet != Sheet::No {
        // the status bar stays as the screen below set it
        src.push_str(".onAppear { m.start() }\n}\n}\n");
    } else {
        src.push_str(&format!(".onAppear {{ StatusBarLook.set(light: {light}); m.start() }}\n}}\n}}\n"));
    }
    indent(&src, 0)
}

/// The navigation bar's look: hidden, or in the colours picked. The title
/// gets its own colour through a principal item, which iOS has no setting for;
/// the back button stays the system's (it takes no tint), and the status
/// bar follows the bar through StatusBarLook.
fn swift_app_bar(s: &Screen) -> String {
    let bar = &s.look();
    if bar.hidden {
        // the page's colour goes on up behind the status bar
        let under = if bar.color != 0 { format!(".background(Color(argb: 0x{:08X}).ignoresSafeArea())\n", bar.color) } else { String::new() };
        return format!("{under}.toolbar(.hidden, for: .navigationBar)\n");
    }
    let mut out = String::new();
    if bar.color != 0 {
        let scheme = if is_light(bar.bar(true)) { "light" } else { "dark" };
        out.push_str(&format!(".toolbarBackground(Color(argb: 0x{:08X}), for: .navigationBar)\n.toolbarBackground(.visible, for: .navigationBar)\n.toolbarColorScheme(.{scheme}, for: .navigationBar)\n", bar.bar(true)));
    }
    if !bar.actions.is_empty() {
        let tint = if bar.color != 0 || bar.title_color != 0 { format!(".foregroundStyle(Color(argb: 0x{:08X}))", bar.title(true)) } else { String::new() };
        out.push_str(".toolbar {\nToolbarItemGroup(placement: .topBarTrailing) {\n");
        for a in &bar.actions {
            let label = crate::codegen::quote_for(if a.label.is_empty() { &a.icon } else { &a.label }, Lang::Swift);
            let run = if has_event(s, &a.id, "onClick") { format!(" m.{}_onClick() ", a.id) } else { " ".into() };
            out.push_str(&format!("Button {{{run}}} label: {{ Image(systemName: \"{}\"){tint} }}\n.accessibilityLabel({label})\n", bar_icon(&a.icon).1));
        }
        out.push_str("}\n}\n");
    }
    if bar.color != 0 || bar.title_color != 0 {
        out.push_str(&format!(".toolbar {{\nToolbarItem(placement: .principal) {{\nText(m.screenTitle).font(.headline).foregroundColor(Color(argb: 0x{:08X}))\n}}\n}}\n", bar.title(true)));
    }
    out
}

fn swift_type_name(app_name: &str) -> String {
    let mut out = String::new();
    let mut up = true;
    for c in app_name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(if up { c.to_ascii_uppercase() } else { c });
            up = false;
        } else {
            up = true;
        }
    }
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert_str(0, "My");
    }
    out
}

/// The iPhone app's entry point: a navigation stack over the screens.
pub fn swift_app(p: &Project) -> String {
    let first = p.screens.first().map(swift_name).unwrap_or_else(|| "Main".into());
    let mut src = format!(
        "import SwiftUI\n\n@main\nstruct {}App: App {{\n@StateObject private var router = Router()\n\nvar body: some Scene {{\nWindowGroup {{\nRootView(router: router)\n}}\n}}\n}}\n\n/// The screens: pushed pages, a bottom sheet over them, and a message from a closed sheet.\nstruct RootView: View {{\n@ObservedObject var router: Router\n\nvar body: some View {{\nNavigationStack(path: $router.path) {{\n{first}Screen(router: router)\n.navigationDestination(for: Route.self) {{ route in\nScreenFor(route: route, router: router)\n}}\n}}\n.sheet(item: $router.sheet) {{ route in\nScreenFor(route: route, router: router)\n}}\n.overlay(alignment: .bottom) {{ ToastView(text: router.toastText) }}\n}}\n}}\n\n",
        swift_type_name(&p.app_name)
    );
    let sheets: Vec<String> = p.screens.iter().filter(|s| p.sheet_of(s) != Sheet::No).map(|s| format!("\"{}\"", s.activity_name())).collect();
    src.push_str(&format!("extension Router {{\n/// The screens that open as bottom sheets.\nstatic let sheets: Set<String> = [{}]\n}}\n\n", sheets.join(", ")));
    src.push_str("/// Opens the screen an Intent names.\nstruct ScreenFor: View {\nlet route: Route\nlet router: Router\n\nvar body: some View {\nswitch route.screen {\n");
    for s in p.screens.iter().skip(1) {
        src.push_str(&format!("case \"{}\":\n{}Screen(router: router, extras: route.extras)\n", s.activity_name(), swift_name(s)));
    }
    src.push_str(&format!("default:\n{first}Screen(router: router, extras: route.extras)\n}}\n}}\n}}\n"));
    indent(&src, 0)
}

/// Shared pieces every generated iPhone screen uses.
pub fn swift_helpers(p: &Project) -> String {
    let base = r#"import SwiftUI
import UIKit
import WebKit
import AVFoundation
import AVKit
import CoreText
import PhotosUI
import UniformTypeIdentifiers

/// Everything a block can change about a widget.
struct WState {
    var text = ""
    var hint = ""
    var enabled = true
    /// 0 visible, 1 invisible, 2 gone
    var visibility = 0
    var checked = false
    var progress = 0.0
    var max = 100.0
    var alpha = 1.0
    var bg: UInt32 = 0x00FFFFFF
    var textColor: UInt32 = 0xFF000000
    var textSize = 12.0
    var items: [String] = []
    var selected = 0
    var url = ""
    var date = Date()
    var radius = 0.0
    var elevation = 0.0
    var border = 0.0
    var borderColor: UInt32 = 0xFF9E9E9E
    var image = ""
    var font = ""
}

/// A web request component: the headers every request sends.
final class WebClient {
    var headers: [String: String] = [:]
}

/// Which screen to open next, and what to hand it.
struct Intent {
    var screen = ""
    var extras: [String: String] = [:]
}

struct DialogState {
    var shown = false
    var title = ""
    var message = ""
    var okLabel: String? = nil
    var ok: (() -> Void)? = nil
    var cancelLabel: String? = nil
    var cancel: (() -> Void)? = nil
}

struct Route: Hashable, Identifiable {
    let id = UUID()
    let screen: String
    let extras: [String: String]
}

final class Router: ObservableObject {
    @Published var path: [Route] = []
    /// The bottom sheet showing over the screens, if any.
    @Published var sheet: Route? = nil
    /// A message from a sheet that closed (a screen's own show over it).
    @Published var toastText: String? = nil

    func showToast(_ text: String) {
        toastText = text
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { [weak self] in
            if self?.toastText == text { self?.toastText = nil }
        }
    }
}

/// Saved data (the Android SharedPreferences), kept in UserDefaults.
struct Prefs {
    let name: String
    func get(_ key: String) -> String { UserDefaults.standard.string(forKey: name + "." + key) ?? "" }
    func set(_ key: String, _ value: String) { UserDefaults.standard.set(value, forKey: name + "." + key) }
    func remove(_ key: String) { UserDefaults.standard.removeObject(forKey: name + "." + key) }
    func contains(_ key: String) -> Bool { UserDefaults.standard.object(forKey: name + "." + key) != nil }
}

/// The status bar's text over a coloured navigation bar: light on a dark
/// bar. SwiftUI's navigation controller decides it and has no setting, so
/// its answer is replaced by the one the showing screen asks for (nil: the
/// system's own).
enum StatusBarLook {
    static var light: Bool? = nil
    private static var done = Set<ObjectIdentifier>()
    static func set(light l: Bool?) {
        light = l
        DispatchQueue.main.async {
            guard let root = UIApplication.shared.connectedScenes.compactMap({ ($0 as? UIWindowScene)?.keyWindow }).first?.rootViewController else { return }
            var vc: UIViewController? = root
            while let v = vc {
                let cls: AnyClass = type(of: v)
                if v is UINavigationController && !done.contains(ObjectIdentifier(cls)) {
                    done.insert(ObjectIdentifier(cls))
                    let answer: @convention(block) (AnyObject) -> Int = { _ in
                        switch StatusBarLook.light {
                        case .some(true): return UIStatusBarStyle.lightContent.rawValue
                        case .some(false): return UIStatusBarStyle.darkContent.rawValue
                        case .none: return UIStatusBarStyle.default.rawValue
                        }
                    }
                    class_replaceMethod(cls, #selector(getter: UIViewController.preferredStatusBarStyle), imp_implementationWithBlock(answer), "q@:")
                }
                v.setNeedsStatusBarAppearanceUpdate()
                vc = v.childForStatusBarStyle
            }
        }
    }
}

enum Helpers {
    /// Android's startsWith and endsWith: letter by letter (UTF-16), no merging of accents.
    static func starts(_ s: String, _ p: String) -> Bool {
        Array(s.utf16).starts(with: Array(p.utf16))
    }
    static func ends(_ s: String, _ p: String) -> Bool {
        let a = Array(s.utf16), b = Array(p.utf16)
        return a.count >= b.count && Array(a[(a.count - b.count)...]) == b
    }
    /// Whether text reads as a number (what "number from" accepts).
    static func isNumber(_ s: String) -> Bool {
        Double(s.trimmingCharacters(in: .whitespacesAndNewlines)) != nil
    }
    /// A number with so many decimals, the way Android rounds it: the number
    /// as Android writes it, halves away from zero ("2.675" → "2.68").
    static func fixed(_ x: Double, _ decimals: Double) -> String {
        let t = decimalText(x)
        if x.isNaN || x.isInfinite { return t }
        let d = decimals.isNaN ? 0 : Int(Swift.min(Swift.max(decimals, 0), 10))
        var s = Substring(t)
        var neg = false
        if s.hasPrefix("-") { neg = true; s = s.dropFirst() }
        var exp = 0
        if let e = s.firstIndex(of: "E") { exp = Int(s[s.index(after: e)...]) ?? 0; s = s[..<e] }
        let dot = s.firstIndex(of: ".").map { s.distance(from: s.startIndex, to: $0) } ?? s.count
        var digits = s.filter { $0 != "." }.compactMap { $0.wholeNumberValue }
        var point = dot + exp
        if point < 0 { digits = Array(repeating: 0, count: -point) + digits; point = 0 }
        if point > digits.count { digits += Array(repeating: 0, count: point - digits.count) }
        let keep = point + d
        if digits.count > keep {
            let up = digits[keep] >= 5
            digits = Array(digits[..<keep])
            if up {
                var i = keep - 1
                while i >= 0 && digits[i] == 9 { digits[i] = 0; i -= 1 }
                if i >= 0 { digits[i] += 1 } else { digits.insert(1, at: 0); point += 1 }
            }
        } else {
            digits += Array(repeating: 0, count: keep - digits.count)
        }
        var whole = digits[..<point].map(String.init).joined()
        let frac = digits[point...].map(String.init).joined()
        while whole.count > 1 && whole.hasPrefix("0") { whole.removeFirst() }
        if whole.isEmpty { whole = "0" }
        let zero = !digits.contains { $0 != 0 }
        return (neg && !zero ? "-" : "") + whole + (d > 0 ? "." + frac : "")
    }
    /// Text cut at each separator (no separator: the whole text).
    static func split(_ s: String, _ sep: String) -> [String] {
        sep.isEmpty ? [s] : s.components(separatedBy: sep)
    }
    static func openLink(_ url: String) {
        if let u = URL(string: url) { UIApplication.shared.open(u) }
    }
    static func share(_ text: String) {
        let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        guard var top = scenes.flatMap({ $0.windows }).first(where: { $0.isKeyWindow })?.rootViewController else { return }
        while let p = top.presentedViewController { top = p }
        top.present(UIActivityViewController(activityItems: [text], applicationActivities: nil), animated: true)
    }
    static func hideKeyboard() {
        UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
    }

    /// The phone's own letters, or one of the app's fonts (cat.ttf or cat.otf) by name.
    static func font(_ name: String, size: Double, bold: Bool) -> Font {
        let weight: Font.Weight = bold ? .bold : .regular
        guard let ps = fontName(name) else { return .system(size: size, weight: weight) }
        return Font.custom(ps, fixedSize: size).weight(weight)
    }
    private static var fontNames: [String: String] = [:]
    /// Loads a font file once and gives the name iOS knows it by.
    private static func fontName(_ name: String) -> String? {
        if name.isEmpty { return nil }
        if let n = fontNames[name] { return n }
        for ext in ["ttf", "otf"] {
            if let url = Bundle.main.url(forResource: name, withExtension: ext),
               let data = CGDataProvider(url: url as CFURL), let cg = CGFont(data), let ps = cg.postScriptName as String? {
                CTFontManagerRegisterGraphicsFont(cg, nil)
                fontNames[name] = ps
                return ps
            }
        }
        return nil
    }

    /// The text of one of the app's data files by name ("" if there is none).
    static func assetText(_ name: String) -> String {
        for ext in ["txt", "json", "csv", "xml", "html", "md"] {
            if let p = Bundle.main.path(forResource: name, ofType: ext), let d = FileManager.default.contents(atPath: p) {
                var t = String(decoding: d, as: UTF8.self)
                if t.hasPrefix("\u{FEFF}") { t.removeFirst() }
                return t
            }
        }
        return ""
    }

    /// A file path (camera, phone) or web address rather than one of the app's own files.
    static func outside(_ name: String) -> URL? {
        if name.hasPrefix("/") { return URL(fileURLWithPath: name) }
        if name.hasPrefix("http://") || name.hasPrefix("https://") || name.hasPrefix("file:") { return URL(string: name) }
        return nil
    }
    private static let pictureFiles = NSCache<NSString, UIImage>()

    /// One of the app's pictures by name (cat.png or cat.jpg), at 1 point a
    /// pixel, or a picture file (web addresses are PictureView's).
    static func picture(_ name: String) -> UIImage? {
        if name.isEmpty { return nil }
        if let u = outside(name) {
            guard u.isFileURL else { return nil }
            if let img = pictureFiles.object(forKey: u.path as NSString) { return img }
            guard let img = UIImage(contentsOfFile: u.path) else { return nil }
            pictureFiles.setObject(img, forKey: u.path as NSString)
            return img
        }
        for ext in ["png", "jpg"] {
            if let p = Bundle.main.path(forResource: name, ofType: ext), let img = UIImage(contentsOfFile: p) {
                return img
            }
        }
        return nil
    }

    /// A whole number from a to b, both included.
    static func random(_ a: Int, _ b: Int) -> Double {
        a <= b ? Double(Int.random(in: a...b)) : Double(a)
    }
    // Text positions count UTF-16 units, as on Android (an emoji is 2).
    static func indexOf(_ part: String, in s: String) -> Double {
        if part.isEmpty { return 0 }
        guard let r = s.range(of: part) else { return -1 }
        return Double(s.utf16.distance(from: s.startIndex, to: r.lowerBound))
    }
    static func lastIndexOf(_ part: String, in s: String) -> Double {
        if part.isEmpty { return Double(s.utf16.count) }
        guard let r = s.range(of: part, options: .backwards) else { return -1 }
        return Double(s.utf16.distance(from: s.startIndex, to: r.lowerBound))
    }
    /// Like Android, a substring outside the text stops the app with the reason.
    static func substring(_ s: String, _ from: Int, _ to: Int) -> String {
        let units = Array(s.utf16)
        if from < 0 || to > units.count || from > to {
            fatalError("StringIndexOutOfBounds: substring \(from) to \(to) is outside the text (length \(units.count))")
        }
        return String(decoding: units[from..<to], as: UTF16.self)
    }
    /// Like Android, text that isn't a number stops the app with the reason.
    static func number(_ s: String) -> Double {
        let t = s.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let v = Double(t) else {
            fatalError(t.isEmpty ? "NumberFormatException: number from empty text" : "NumberFormatException: \"\(s)\" is not a number")
        }
        return v
    }
    /// Kotlin's contains: every text contains the empty text.
    static func contains(_ s: String, _ part: String) -> Bool {
        part.isEmpty || s.contains(part)
    }
    /// A number as text the way Android writes it (Java's Double.toString):
    /// 7.0, 0.001, 1234567.5, 1.0E7, 1.5E-6, NaN, Infinity.
    static func decimalText(_ v: Double) -> String {
        if v.isNaN { return "NaN" }
        if v.isInfinite { return v < 0 ? "-Infinity" : "Infinity" }
        if v == 0 { return v.sign == .minus ? "-0.0" : "0.0" }
        let a = Swift.abs(v)
        // Swift's own shortest digits ("1e+20", "1.5e-06", "1234567.5"), re-placed
        var t = "\(a)"
        var exp = 0
        if let e = t.firstIndex(where: { $0 == "e" || $0 == "E" }) {
            exp = Int(t[t.index(after: e)...]) ?? 0
            t = String(t[..<e])
        }
        let parts = t.split(separator: ".", omittingEmptySubsequences: false)
        let whole = String(parts[0])
        var digits = whole + (parts.count > 1 ? String(parts[1]) : "")
        exp += whole.count - 1
        while digits.count > 1 && digits.hasPrefix("0") { digits.removeFirst(); exp -= 1 }
        while digits.count > 1 && digits.hasSuffix("0") { digits.removeLast() }
        let sign = v < 0 ? "-" : ""
        if a >= 1e-3 && a < 1e7 {
            if exp >= 0 {
                let cut = exp + 1
                if digits.count > cut { return sign + String(digits.prefix(cut)) + "." + String(digits.dropFirst(cut)) }
                return sign + digits + String(repeating: "0", count: cut - digits.count) + ".0"
            }
            return sign + "0." + String(repeating: "0", count: -exp - 1) + digits
        }
        return sign + String(digits.prefix(1)) + "." + (digits.count > 1 ? String(digits.dropFirst()) : "0") + "E" + String(exp)
    }
    static func dip(_ n: Int) -> Double { Double(n) * Double(UIScreen.main.scale) }
    static var screenWidth: Double { Double(UIScreen.main.nativeBounds.width) }
    static var screenHeight: Double { Double(UIScreen.main.nativeBounds.height) }
    static func format(_ date: Date, _ pattern: String) -> String {
        let f = DateFormatter()
        f.dateFormat = pattern
        return f.string(from: date)
    }
    static func vibrate() { UIImpactFeedbackGenerator(style: .medium).impactOccurred() }
    static func after(_ ms: Double, _ body: @escaping () -> Void) -> Timer {
        Timer.scheduledTimer(withTimeInterval: Swift.max(ms, 0) / 1000, repeats: false) { _ in body() }
    }
    /// A web request; `done` runs on the main thread with (answered with a
    /// 2xx code, the code or 0, the answer's text or what went wrong).
    static func web(_ c: WebClient, _ method: String, _ url: String, _ body: String, _ done: @escaping (Bool, Double, String) -> Void) {
        guard let u = URL(string: url.trimmingCharacters(in: .whitespaces)), u.scheme != nil else {
            done(false, 0, "Not a web address: \(url)")
            return
        }
        var r = URLRequest(url: u, timeoutInterval: 30)
        r.httpMethod = method
        for (k, v) in c.headers { r.setValue(v, forHTTPHeaderField: k) }
        if !body.isEmpty && method != "GET" {
            r.httpBody = body.data(using: .utf8)
            if r.value(forHTTPHeaderField: "Content-Type") == nil { r.setValue("application/json", forHTTPHeaderField: "Content-Type") }
        }
        URLSession.shared.dataTask(with: r) { data, resp, err in
            let code = (resp as? HTTPURLResponse)?.statusCode ?? 0
            let text = err.map { $0.localizedDescription } ?? String(decoding: data ?? Data(), as: UTF8.self)
            DispatchQueue.main.async { done(err == nil && (200..<300).contains(code), Double(code), text) }
        }.resume()
    }
    /// A number without its decimals, as Kotlin's toLong() does it: NaN is 0
    /// and numbers too big stop at the largest whole number (no crash).
    static func wholeText(_ v: Double) -> String {
        if v.isNaN { return "0" }
        if v >= 9.223372036854775807e18 { return "9223372036854775807" }
        if v <= -9.223372036854775808e18 { return "-9223372036854775808" }
        return String(Int64(v))
    }
    /// JSON values as the text blocks show them (nested ones stay JSON).
    static func jsonText(_ v: Any) -> String {
        if v is NSNull { return "" }
        if let s = v as? String { return s }
        if let n = v as? NSNumber {
            if CFGetTypeID(n) == CFBooleanGetTypeID() { return n.boolValue ? "true" : "false" }
            return n.stringValue
        }
        return toJson(v)
    }
    static func jsonToMap(_ text: String) -> [String: Any] {
        guard let d = text.data(using: .utf8), let o = (try? JSONSerialization.jsonObject(with: d)) as? [String: Any] else { return [:] }
        return o.mapValues { jsonText($0) }
    }
    static func jsonToListMap(_ text: String) -> [[String: Any]] {
        guard let d = text.data(using: .utf8), let o = try? JSONSerialization.jsonObject(with: d) else { return [] }
        let items: [Any] = (o as? [Any]) ?? [o]
        return items.map { item in
            if let m = item as? [String: Any] { return m.mapValues { jsonText($0) } }
            return ["value": jsonText(item)]
        }
    }
    static func toJson(_ v: Any) -> String {
        // keys in order and "/" left alone, as on Android and in the preview
        guard JSONSerialization.isValidJSONObject(v), let d = try? JSONSerialization.data(withJSONObject: v, options: [.sortedKeys, .withoutEscapingSlashes]) else { return "" }
        return String(decoding: d, as: UTF8.self)
    }
    static func every(first: Double, period: Double, _ body: @escaping () -> Void) -> Timer {
        let t = Timer(fire: Date().addingTimeInterval(Swift.max(first, 0) / 1000), interval: Swift.max(period, 1) / 1000, repeats: true) { _ in body() }
        RunLoop.main.add(t, forMode: .common)
        return t
    }
}

extension Color {
    /// An Android colour number (0xAARRGGBB).
    init(argb: UInt32) {
        self.init(.sRGB,
                  red: Double((argb >> 16) & 0xFF) / 255,
                  green: Double((argb >> 8) & 0xFF) / 255,
                  blue: Double(argb & 0xFF) / 255,
                  opacity: Double((argb >> 24) & 0xFF) / 255)
    }
}

private struct LinearWeight: LayoutValueKey { static let defaultValue: CGFloat = 0 }
private struct LinearMargin: LayoutValueKey { static let defaultValue: CGFloat = 0 }

extension View {
    /// How a child of a `Linear` takes part: its weight and its margin.
    func linearItem(weight: CGFloat, margin: CGFloat) -> some View {
        layoutValue(key: LinearWeight.self, value: weight).layoutValue(key: LinearMargin.self, value: margin)
    }
}

/// A row or column as an Android LinearLayout lays it out: children without a
/// weight take their own size, and the room left is shared by weight
/// (a weight of 2 gets twice what a weight of 1 gets).
struct Linear: Layout {
    let vertical: Bool
    /// Where children sit across: 0 start, 0.5 centre, 1 end.
    let cross: CGFloat

    private func along(_ s: CGSize) -> CGFloat { vertical ? s.height : s.width }
    private func across(_ s: CGSize) -> CGFloat { vertical ? s.width : s.height }
    private func offer(_ along: CGFloat?, _ across: CGFloat?) -> ProposedViewSize {
        vertical ? ProposedViewSize(width: across, height: along) : ProposedViewSize(width: along, height: across)
    }
    private func finite(_ v: CGFloat?) -> CGFloat? { v.flatMap { $0.isFinite ? $0 : nil } }

    /// Each child's length along the line.
    private func lengths(_ proposal: ProposedViewSize, _ subviews: Subviews) -> [CGFloat] {
        let total = finite(vertical ? proposal.height : proposal.width)
        let side = finite(vertical ? proposal.width : proposal.height)
        var out = [CGFloat](repeating: 0, count: subviews.count)
        var used: CGFloat = 0
        var weights: CGFloat = 0
        for (i, v) in subviews.enumerated() {
            if total != nil && v[LinearWeight.self] > 0 {
                weights += v[LinearWeight.self]
                used += 2 * v[LinearMargin.self]
            } else {
                out[i] = along(v.sizeThatFits(offer(nil, side)))
                used += out[i]
            }
        }
        if let total, weights > 0 {
            let spare = Swift.max(0, total - used)
            for (i, v) in subviews.enumerated() where v[LinearWeight.self] > 0 {
                out[i] = spare * v[LinearWeight.self] / weights + 2 * v[LinearMargin.self]
            }
        }
        return out
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let lens = lengths(proposal, subviews)
        let side = finite(vertical ? proposal.width : proposal.height)
        var widest: CGFloat = 0
        for (i, v) in subviews.enumerated() {
            widest = Swift.max(widest, across(v.sizeThatFits(offer(lens[i], side))))
        }
        let length = lens.reduce(0, +)
        return vertical ? CGSize(width: widest, height: length) : CGSize(width: length, height: widest)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let lens = lengths(ProposedViewSize(bounds.size), subviews)
        let side = across(bounds.size)
        var at = vertical ? bounds.minY : bounds.minX
        for (i, v) in subviews.enumerated() {
            let size = v.sizeThatFits(offer(lens[i], side))
            let off = (side - across(size)) * cross
            let point = vertical ? CGPoint(x: bounds.minX + off, y: at) : CGPoint(x: at, y: bounds.minY + off)
            v.place(at: point, anchor: .topLeading, proposal: offer(lens[i], side))
            at += lens[i]
        }
    }
}

enum SizeRule {
    case wrap, fill, fixed(CGFloat)
    var fixedValue: CGFloat? { if case .fixed(let v) = self { return v } else { return nil } }
    var isFill: Bool { if case .fill = self { return true } else { return false } }
}

extension View {
    /// Size, padding, background, see-through and on/off, as the Android layout has them.
    func widgetBox(_ s: WState, width: SizeRule, height: SizeRule, padding: CGFloat, margin: CGFloat, align: Alignment) -> some View {
        self.padding(padding)
            .frame(width: width.fixedValue, height: height.fixedValue, alignment: align)
            .frame(maxWidth: width.isFill ? .infinity : nil, maxHeight: height.isFill ? .infinity : nil, alignment: align)
            .modifier(Corners(radius: s.radius))
            .background(
                RoundedRectangle(cornerRadius: s.radius)
                    .fill(Color(argb: s.bg))
                    .shadow(color: .black.opacity(s.elevation > 0 ? 0.3 : 0), radius: s.elevation / 2, x: 0, y: s.elevation / 2)
            )
            .overlay(RoundedRectangle(cornerRadius: s.radius).strokeBorder(Color(argb: s.borderColor), lineWidth: s.border))
            .opacity(s.visibility == 1 ? 0 : s.alpha)
            .disabled(!s.enabled)
            .padding(margin)
    }
}

/// A password field, with an eye at its end that shows or hides what's typed.
struct PasswordField: View {
    let hint: String
    @Binding var text: String
    @State private var shown = false
    var body: some View {
        // a hidden line under it: the field keeps its height shown or not
        ZStack(alignment: .leading) {
            TextField("", text: .constant("")).hidden().accessibilityHidden(true)
            if shown {
                TextField(hint, text: $text)
            } else {
                SecureField(hint, text: $text)
            }
        }
        .textInputAutocapitalization(.never)
        .autocorrectionDisabled()
        .padding(.vertical, 7)
        .padding(.leading, 8)
        .padding(.trailing, 36)
        .background(RoundedRectangle(cornerRadius: 6).fill(Color(.systemBackground)))
        .overlay(RoundedRectangle(cornerRadius: 6).stroke(Color(.systemGray4), lineWidth: 0.5))
        .overlay(alignment: .trailing) {
            Button { shown.toggle() } label: {
                Image(systemName: shown ? "eye.slash" : "eye").foregroundStyle(.secondary).frame(width: 32, height: 28).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(shown ? "Hide password" : "Show password")
        }
    }
}

/// A picture the app carries, by name ("cat" is cat.png or cat.jpg in the
/// app). Its own size is its size in points, as on Android (1 pixel, 1 dp).
struct PictureView: View {
    let name: String
    let crop: Bool
    let natural: Bool
    var fill = false
    var body: some View {
        if let u = Helpers.outside(name), !u.isFileURL {
            AsyncImage(url: u) { phase in
                if let img = phase.image {
                    if crop {
                        Color.clear.overlay(img.resizable().scaledToFill()).clipped()
                    } else if fill {
                        Color.clear.overlay(img.resizable().scaledToFit())
                    } else {
                        img.resizable().aspectRatio(contentMode: .fit)
                    }
                } else {
                    Color.clear
                }
            }
        } else if let img = Helpers.picture(name) {
            if natural {
                Image(uiImage: img).resizable().aspectRatio(contentMode: .fit)
                    .frame(maxWidth: img.size.width, maxHeight: img.size.height)
            } else if crop {
                Color.clear.overlay(Image(uiImage: img).resizable().scaledToFill()).clipped()
            } else if fill {
                Color.clear.overlay(Image(uiImage: img).resizable().scaledToFit())
            } else {
                Image(uiImage: img).resizable().aspectRatio(contentMode: .fit)
            }
        } else {
            Color.clear.frame(width: 0, height: 0)
        }
    }
}

/// The camera and the phone's photos and files. [done] gets (got a file,
/// the file's path in the app's own files); the simulator has no camera,
/// so there it is always "cancelled".
final class MediaPicker: NSObject, UIImagePickerControllerDelegate, UINavigationControllerDelegate, PHPickerViewControllerDelegate, UIDocumentPickerDelegate {
    static let shared = MediaPicker()
    private var done: ((Bool, String) -> Void)?
    private var kind = ""

    private static func top() -> UIViewController? {
        let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        guard var top = scenes.flatMap({ $0.windows }).first(where: { $0.isKeyWindow })?.rootViewController else { return nil }
        while let p = top.presentedViewController { top = p }
        return top
    }
    private static func newFile(_ ext: String) -> URL {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("media", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.appendingPathComponent("m\(Int(Date().timeIntervalSince1970 * 1000)).\(ext.isEmpty ? "dat" : ext.lowercased())")
    }
    private static func copy(_ from: URL, _ ext: String) -> String? {
        let f = newFile(ext)
        return (try? FileManager.default.copyItem(at: from, to: f)) != nil ? f.path : nil
    }
    private func finish(_ path: String?) {
        let d = done
        done = nil
        DispatchQueue.main.async { d?(path != nil, path ?? "") }
    }
    private func begin(_ done: @escaping (Bool, String) -> Void) -> UIViewController? {
        finish(nil)
        guard let top = Self.top() else {
            DispatchQueue.main.async { done(false, "") }
            return nil
        }
        self.done = done
        return top
    }

    func camera(video: Bool, _ done: @escaping (Bool, String) -> Void) {
        // no camera, or one that can't take this kind (the simulator's has no video)
        let type = (video ? UTType.movie : UTType.image).identifier
        guard UIImagePickerController.isSourceTypeAvailable(.camera),
              UIImagePickerController.availableMediaTypes(for: .camera)?.contains(type) == true else {
            DispatchQueue.main.async { done(false, "") }
            return
        }
        guard let top = begin(done) else { return }
        let p = UIImagePickerController()
        p.sourceType = .camera
        p.mediaTypes = [type]
        if video { p.cameraCaptureMode = .video }
        p.delegate = self
        top.present(p, animated: true)
    }
    func imagePickerController(_ picker: UIImagePickerController, didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]) {
        picker.dismiss(animated: true)
        if let u = info[.mediaURL] as? URL {
            finish(Self.copy(u, u.pathExtension.isEmpty ? "mov" : u.pathExtension))
        } else if let img = info[.originalImage] as? UIImage, let d = img.jpegData(compressionQuality: 0.9) {
            let f = Self.newFile("jpg")
            finish((try? d.write(to: f)) != nil ? f.path : nil)
        } else {
            finish(nil)
        }
    }
    func imagePickerControllerDidCancel(_ picker: UIImagePickerController) {
        picker.dismiss(animated: true)
        finish(nil)
    }

    /// "picture" and "video" from Photos, "sound" from Files.
    func pick(_ kind: String, _ done: @escaping (Bool, String) -> Void) {
        guard let top = begin(done) else { return }
        self.kind = kind
        if kind == "sound" {
            let p = UIDocumentPickerViewController(forOpeningContentTypes: [.audio], asCopy: true)
            p.delegate = self
            top.present(p, animated: true)
            return
        }
        var c = PHPickerConfiguration()
        c.filter = kind == "video" ? .videos : .images
        c.selectionLimit = 1
        c.preferredAssetRepresentationMode = .current
        let p = PHPickerViewController(configuration: c)
        p.delegate = self
        top.present(p, animated: true)
    }
    func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
        picker.dismiss(animated: true)
        let want: UTType = kind == "video" ? .movie : .image
        guard let item = results.first?.itemProvider,
              let id = item.registeredTypeIdentifiers.first(where: { UTType($0)?.conforms(to: want) == true }) else {
            finish(nil)
            return
        }
        let ext = UTType(id)?.preferredFilenameExtension ?? (want == .movie ? "mov" : "jpg")
        item.loadFileRepresentation(forTypeIdentifier: id) { url, _ in
            guard let url else { return self.finish(nil) }
            // HEIC photos become JPEG, which every phone and website can show
            if want == .image, ["heic", "heif"].contains(ext.lowercased()) {
                let f = Self.newFile("jpg")
                if let img = UIImage(contentsOfFile: url.path), let d = img.jpegData(compressionQuality: 0.9), (try? d.write(to: f)) != nil {
                    return self.finish(f.path)
                }
            }
            self.finish(Self.copy(url, ext))
        }
    }
    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        guard let u = urls.first else { return finish(nil) }
        finish(Self.copy(u, u.pathExtension.isEmpty ? "m4a" : u.pathExtension))
    }
    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        finish(nil)
    }
}

/// A sound player: plays one of the app's sounds at a time.
final class SoundPlayer {
    private var player: AVAudioPlayer?
    private var looping = false
    private var volume: Float = 1
    private var asked = UUID()
    /// One of the app's sounds by name, a sound file, or a web address (fetched first).
    func play(_ name: String) {
        stop()
        if let u = Helpers.outside(name) {
            if u.isFileURL {
                start(try? AVAudioPlayer(contentsOf: u))
            } else {
                let now = asked
                URLSession.shared.dataTask(with: u) { d, _, _ in
                    DispatchQueue.main.async { [weak self] in
                        guard let self, self.asked == now, let d else { return }
                        self.start(try? AVAudioPlayer(data: d))
                    }
                }.resume()
            }
            return
        }
        guard let url = ["mp3", "wav", "m4a"].lazy.compactMap({ Bundle.main.url(forResource: name, withExtension: $0) }).first else { return }
        start(try? AVAudioPlayer(contentsOf: url))
    }
    private func start(_ p: AVAudioPlayer?) {
        try? AVAudioSession.sharedInstance().setCategory(.playback)
        player = p
        player?.numberOfLoops = looping ? -1 : 0
        player?.volume = volume
        player?.play()
    }
    func pause() { player?.pause() }
    func resume() { player?.play() }
    func stop() { asked = UUID(); player?.stop(); player = nil }
    func setLooping(_ on: Bool) { looping = on; player?.numberOfLoops = on ? -1 : 0 }
    func setVolume(_ percent: Double) { volume = Float(Swift.min(Swift.max(percent / 100, 0), 1)); player?.volume = volume }
    func seek(_ ms: Double) { player?.currentTime = Swift.max(ms, 0) / 1000 }
    var isPlaying: Bool { player?.isPlaying ?? false }
    var position: Double { ((player?.currentTime ?? 0) * 1000).rounded() }
    var duration: Double { ((player?.duration ?? 0) * 1000).rounded() }
}

/// A video widget's player: plays one of the app's videos (cat.mp4, .m4v or .mov).
final class VideoControl {
    let player = AVPlayer()
    var onEnd: (() -> Void)?
    private var looping = false
    private var ended: NSObjectProtocol?
    init() {
        // the videos are in the app: nothing to wait for
        player.automaticallyWaitsToMinimizeStalling = false
        ended = NotificationCenter.default.addObserver(forName: .AVPlayerItemDidPlayToEndTime, object: nil, queue: .main) { [weak self] note in
            guard let self, (note.object as? AVPlayerItem) === self.player.currentItem else { return }
            if self.looping {
                self.player.seek(to: .zero)
                self.player.play()
            } else {
                self.onEnd?()
            }
        }
    }
    deinit {
        if let e = ended { NotificationCenter.default.removeObserver(e) }
        player.pause()
    }
    func load(_ name: String) {
        guard !name.isEmpty, let url = Helpers.outside(name) ?? ["mp4", "m4v", "mov"].lazy.compactMap({ Bundle.main.url(forResource: name, withExtension: $0) }).first else {
            player.replaceCurrentItem(with: nil)
            return
        }
        try? AVAudioSession.sharedInstance().setCategory(.playback)
        try? AVAudioSession.sharedInstance().setActive(true)
        player.replaceCurrentItem(with: AVPlayerItem(url: url))
    }
    func play() { player.play() }
    func pause() { player.pause() }
    func seek(_ ms: Double) { player.seek(to: CMTime(value: CMTimeValue(Swift.max(ms, 0)), timescale: 1000), toleranceBefore: .zero, toleranceAfter: .zero) }
    func setLooping(_ on: Bool) { looping = on }
    /// Playing, not just asked to play (as on Android, false until the video is ready).
    var isPlaying: Bool { player.timeControlStatus == .playing }
    var position: Double { let t = player.currentTime().seconds; return t.isFinite ? (t * 1000).rounded() : 0 }
    var duration: Double { let d = player.currentItem?.duration.seconds ?? 0; return d.isFinite ? (d * 1000).rounded() : 0 }
}

/// The video on screen, with or without the play/pause controls.
struct VideoBox: View {
    let control: VideoControl
    let controls: Bool
    var body: some View {
        if controls {
            VideoPlayer(player: control.player)
        } else {
            VideoLayer(player: control.player)
        }
    }
}

/// Just the moving picture, no controls.
struct VideoLayer: UIViewRepresentable {
    let player: AVPlayer
    final class LayerView: UIView {
        override class var layerClass: AnyClass { AVPlayerLayer.self }
        var playerLayer: AVPlayerLayer { layer as! AVPlayerLayer }
    }
    func makeUIView(context: Context) -> LayerView {
        let v = LayerView()
        v.playerLayer.player = player
        v.playerLayer.videoGravity = .resizeAspect
        return v
    }
    func updateUIView(_ v: LayerView, context: Context) {}
}

/// Rounded corners cut what's inside too (a picture, say); square ones cut nothing.
struct Corners: ViewModifier {
    let radius: Double
    @ViewBuilder func body(content: Content) -> some View {
        if radius > 0 {
            content.clipShape(RoundedRectangle(cornerRadius: radius))
        } else {
            content
        }
    }
}

/// The short message at the bottom (Android's toast).
struct ToastView: View {
    let text: String?
    var body: some View {
        if let t = text {
            Text(t)
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                .background(Capsule().fill(Color.black.opacity(0.8)))
                .foregroundColor(.white)
                .padding(.bottom, 40)
        }
    }
}

struct WebPage: UIViewRepresentable {
    let url: String
    func makeUIView(context: Context) -> WKWebView { WKWebView() }
    func updateUIView(_ view: WKWebView, context: Context) {
        guard let u = URL(string: url), view.url != u else { return }
        view.load(URLRequest(url: u))
    }
}
"#;
    format!("{base}{}", supa_swift(p))
}

// ---------------------------------------------------------------- Kotlin project files

/// The small helper the generated screens call: short message, random whole
/// number with both ends included, dp and screen size (same rules as the Java one).
pub fn kotlin_util(p: &Project) -> String {
    format!(
        r#"package {}

import android.app.Activity
import android.content.Context
import android.util.TypedValue
import android.widget.Toast
import java.net.HttpURLConnection
import java.net.URL
import java.util.Random
import org.json.JSONArray
import org.json.JSONObject

/** A sound player: plays one of the app's sounds (res/raw) at a time. */
class SoundPlayer {{
    private var mp: android.media.MediaPlayer? = null
    private var looping = false
    private var volume = 1f

    fun play(context: Context, name: String) {{
        stop()
        // a file path (camera, phone) or a web address: prepared off the main thread
        if (AppUtil.outside(name)) {{
            val p = android.media.MediaPlayer()
            mp = p
            try {{
                if (name.startsWith("content:")) p.setDataSource(context, android.net.Uri.parse(name)) else p.setDataSource(name.removePrefix("file://"))
                p.isLooping = looping
                p.setVolume(volume, volume)
                p.setOnPreparedListener {{ if (mp === it) it.start() }}
                p.setOnErrorListener {{ _, _, _ -> true }}
                p.prepareAsync()
            }} catch (e: Exception) {{
                p.release()
                mp = null
            }}
            return
        }}
        val id = if (name.isEmpty()) 0 else context.resources.getIdentifier(name, "raw", context.packageName)
        if (id == 0) return
        mp = android.media.MediaPlayer.create(context, id)?.also {{
            it.isLooping = looping
            it.setVolume(volume, volume)
            it.start()
        }}
    }}
    fun pause() {{ mp?.let {{ if (it.isPlaying) it.pause() }} }}
    fun resume() {{ mp?.start() }}
    fun stop() {{ mp?.release(); mp = null }}
    fun setLooping(on: Boolean) {{ looping = on; mp?.isLooping = on }}
    fun setVolume(percent: Double) {{ volume = Math.min(Math.max(percent / 100, 0.0), 1.0).toFloat(); mp?.setVolume(volume, volume) }}
    fun seek(ms: Double) {{ mp?.seekTo(Math.max(ms, 0.0).toInt()) }}
    val isPlaying: Boolean get() = mp?.isPlaying ?: false
    val position: Double get() = (mp?.currentPosition ?: 0).toDouble()
    val duration: Double get() = (mp?.duration ?: 0).toDouble()
}}

/** A video widget's player: plays one of the app's videos (res/raw). */
class VideoControl(private val v: android.widget.VideoView) {{
    private var mp: android.media.MediaPlayer? = null
    private var looping = false
    var onEnd: (() -> Unit)? = null

    init {{
        v.setOnPreparedListener {{ mp = it; it.isLooping = looping }}
        v.setOnCompletionListener {{ onEnd?.invoke() }}
        // no "Can't play this video" box: a video that can't play stays still
        v.setOnErrorListener {{ _, _, _ -> true }}
        setControls(true)
    }}
    fun setControls(on: Boolean) {{
        if (on) {{
            val c = android.widget.MediaController(v.context)
            c.setAnchorView(v)
            v.setMediaController(c)
        }} else {{
            v.setMediaController(null)
        }}
    }}
    fun load(name: String) {{
        mp = null
        if (AppUtil.outside(name)) {{
            v.setVideoURI(if (name.startsWith("/")) android.net.Uri.fromFile(java.io.File(name)) else android.net.Uri.parse(name))
            return
        }}
        val id = if (name.isEmpty()) 0 else v.resources.getIdentifier(name, "raw", v.context.packageName)
        if (id == 0) {{ v.stopPlayback(); return }}
        v.setVideoURI(android.net.Uri.parse("android.resource://" + v.context.packageName + "/" + id))
    }}
    fun play() {{ v.start() }}
    fun pause() {{ if (v.isPlaying) v.pause() }}
    /** To that exact moment (as on iPhone), not the nearest key frame before it. */
    fun seek(ms: Double) {{
        val t = Math.max(ms, 0.0).toInt()
        val p = mp
        if (p != null && android.os.Build.VERSION.SDK_INT >= 26) p.seekTo(t.toLong(), android.media.MediaPlayer.SEEK_CLOSEST) else v.seekTo(t)
    }}
    fun setLooping(on: Boolean) {{ looping = on; mp?.isLooping = on }}
    val isPlaying: Boolean get() = v.isPlaying
    val position: Double get() = if (mp == null) 0.0 else v.currentPosition.toDouble()
    val duration: Double get() = if (mp == null) 0.0 else Math.max(v.duration, 0).toDouble()
}}

/** The camera and the phone's own files. Each asks another app and waits
 *  for its answer (the screen's onActivityResult); [done] gets (got a file,
 *  the file's path in this app's own files). */
object AppMedia {{
    private var next = 7100
    private val waiting = HashMap<Int, (Boolean, android.content.Intent?) -> Unit>()

    @JvmStatic fun onResult(code: Int, result: Int, data: android.content.Intent?): Boolean {{
        val f = waiting.remove(code) ?: return false
        f(result == Activity.RESULT_OK, data)
        return true
    }}

    private fun ask(a: Activity, i: android.content.Intent, cancelled: () -> Unit, answer: (Boolean, android.content.Intent?) -> Unit) {{
        val code = next
        next = if (next >= 7999) 7100 else next + 1
        waiting[code] = answer
        try {{
            a.startActivityForResult(i, code)
        }} catch (e: Exception) {{
            // no camera app, or none for that kind of file
            waiting.remove(code)
            cancelled()
        }}
    }}

    private fun newFile(a: Activity, ext: String): java.io.File {{
        val dir = java.io.File(a.cacheDir, "media")
        dir.mkdirs()
        return java.io.File(dir, "m" + System.currentTimeMillis() + "." + ext)
    }}

    /** What another app gave back, copied into this app's own files; its path or "". */
    private fun copy(a: Activity, uri: android.net.Uri, ext: String): String = try {{
        val f = newFile(a, ext)
        a.contentResolver.openInputStream(uri)?.use {{ input -> f.outputStream().use {{ input.copyTo(it) }} }}
        if (f.length() > 0) f.absolutePath else ""
    }} catch (e: Exception) {{
        ""
    }}

    @JvmStatic fun photo(a: Activity, done: (Boolean, String) -> Unit) = capture(a, android.provider.MediaStore.ACTION_IMAGE_CAPTURE, "jpg", done)

    @JvmStatic fun video(a: Activity, done: (Boolean, String) -> Unit) = capture(a, android.provider.MediaStore.ACTION_VIDEO_CAPTURE, "mp4", done)

    /** The camera app writes straight into this app's files (MediaFiles). */
    private fun capture(a: Activity, action: String, ext: String, done: (Boolean, String) -> Unit) {{
        val f = newFile(a, ext)
        val i = android.content.Intent(action)
        i.putExtra(android.provider.MediaStore.EXTRA_OUTPUT, android.net.Uri.parse("content://" + a.packageName + ".media/" + f.name))
        i.addFlags(android.content.Intent.FLAG_GRANT_WRITE_URI_PERMISSION or android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION)
        ask(a, i, {{ done(false, "") }}) {{ ok, data ->
            val back = data?.data
            when {{
                !ok -> {{ f.delete(); done(false, "") }}
                f.length() > 0 -> done(true, f.absolutePath)
                // a camera app that ignored where to write
                back != null -> Thread {{ val p = copy(a, back, ext); a.runOnUiThread {{ done(p.isNotEmpty(), p) }} }}.start()
                else -> {{
                    val small = data?.extras?.get("data") as? android.graphics.Bitmap
                    if (small != null) {{
                        f.outputStream().use {{ small.compress(android.graphics.Bitmap.CompressFormat.JPEG, 92, it) }}
                        done(true, f.absolutePath)
                    }} else {{
                        f.delete()
                        done(false, "")
                    }}
                }}
            }}
        }}
    }}

    /** A picture, video or sound the person picks from the phone. */
    @JvmStatic fun pick(a: Activity, kind: String, done: (Boolean, String) -> Unit) {{
        val type = when (kind) {{ "video" -> "video"; "sound" -> "audio"; else -> "image" }}
        val i = android.content.Intent(android.content.Intent.ACTION_GET_CONTENT)
        i.type = "$type/*"
        i.addCategory(android.content.Intent.CATEGORY_OPENABLE)
        ask(a, i, {{ done(false, "") }}) {{ ok, data ->
            val uri = data?.data
            if (!ok || uri == null) {{
                done(false, "")
            }} else {{
                val mime = a.contentResolver.getType(uri) ?: ""
                val ext = android.webkit.MimeTypeMap.getSingleton().getExtensionFromMimeType(mime)
                    ?: when (type) {{ "video" -> "mp4"; "audio" -> "mp3"; else -> "jpg" }}
                Thread {{ val p = copy(a, uri, ext); a.runOnUiThread {{ done(p.isNotEmpty(), p) }} }}.start()
            }}
        }}
    }}
}}

/** Lets the camera app write a photo or video into this app's files
 *  (only the one file it was given, for as long as it was given it). */
class MediaFiles : android.content.ContentProvider() {{
    private fun file(uri: android.net.Uri): java.io.File? {{
        val name = uri.lastPathSegment ?: return null
        if (!Regex("m[0-9]+\\.(jpg|mp4)").matches(name)) return null
        return java.io.File(java.io.File(context!!.cacheDir, "media"), name)
    }}
    override fun onCreate(): Boolean = true
    override fun openFile(uri: android.net.Uri, mode: String): android.os.ParcelFileDescriptor? {{
        val f = file(uri) ?: throw java.io.FileNotFoundException(uri.toString())
        return android.os.ParcelFileDescriptor.open(f, android.os.ParcelFileDescriptor.parseMode(mode))
    }}
    override fun getType(uri: android.net.Uri): String = if (uri.toString().endsWith(".mp4")) "video/mp4" else "image/jpeg"
    override fun query(uri: android.net.Uri, projection: Array<out String>?, selection: String?, args: Array<out String>?, order: String?): android.database.Cursor? {{
        val f = file(uri) ?: return null
        val c = android.database.MatrixCursor(arrayOf(android.provider.OpenableColumns.DISPLAY_NAME, android.provider.OpenableColumns.SIZE))
        c.addRow(arrayOf<Any>(f.name, f.length()))
        return c
    }}
    override fun insert(uri: android.net.Uri, values: android.content.ContentValues?): android.net.Uri? = null
    override fun delete(uri: android.net.Uri, selection: String?, args: Array<out String>?): Int = 0
    override fun update(uri: android.net.Uri, values: android.content.ContentValues?, selection: String?, args: Array<out String>?): Int = 0
}}

/** A web request component: the headers every request sends. */
class WebClient {{
    val headers = HashMap<String, String>()
}}

object AppUtil {{
    private val rng = Random()

    /** A password field's eye, at its end: a tap on it shows or hides what's typed. */
    @JvmStatic fun passwordEye(e: android.widget.EditText) {{
        var shown = false
        // a password field's letters are monospace; shown, they look like any text
        if (e.typeface == android.graphics.Typeface.MONOSPACE) e.typeface = android.graphics.Typeface.DEFAULT
        fun update() {{
            val at = e.selectionEnd
            e.transformationMethod = if (shown) null else android.text.method.PasswordTransformationMethod.getInstance()
            e.setCompoundDrawablesRelativeWithIntrinsicBounds(0, 0, if (shown) R.drawable.blockwise_eye_off else R.drawable.blockwise_eye, 0)
            if (at >= 0) e.setSelection(Math.min(at, e.text.length))
        }}
        update()
        e.setOnTouchListener {{ _, ev ->
            val icon = e.compoundDrawablesRelative[2]
            if (icon != null && ev.action == android.view.MotionEvent.ACTION_UP && ev.x >= e.width - e.paddingRight - icon.bounds.width() - 24) {{
                shown = !shown
                update()
                true
            }} else false
        }}
    }}

    /** The view's background as a shape that can have corners and an outline. */
    private fun shape(v: android.view.View): android.graphics.drawable.GradientDrawable {{
        val bg = v.background
        if (bg is android.graphics.drawable.GradientDrawable) return bg
        val g = android.graphics.drawable.GradientDrawable()
        g.setColor(if (bg is android.graphics.drawable.ColorDrawable) bg.color else if (v is android.widget.Button) 0xFFE6E6E6.toInt() else 0)
        v.background = g
        return g
    }}

    private fun dp(v: android.view.View, n: Double): Float = (n * v.resources.displayMetrics.density).toFloat()

    /** Whether text reads as a number (what "number from" accepts). */
    @JvmStatic fun isNumber(s: String): Boolean = try {{ s.toDouble(); true }} catch (e: NumberFormatException) {{ false }}

    /** A number with so many decimals, halves rounded away from zero. */
    @JvmStatic fun fixed(x: Double, decimals: Double): String {{
        if (x.isNaN() || x.isInfinite()) return x.toString()
        val d = Math.min(Math.max(decimals, 0.0), 10.0).toInt()
        return java.math.BigDecimal(x.toString()).setScale(d, java.math.RoundingMode.HALF_UP).toPlainString()
    }}

    /** Text cut at each separator (no separator: the whole text). */
    @JvmStatic fun split(s: String, sep: String): List<String> = if (sep.isEmpty()) listOf(s) else s.split(sep)

    @JvmStatic fun openLink(a: Activity, url: String) {{
        try {{
            a.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(url)))
        }} catch (e: Exception) {{
            toast(a, "Can't open " + url)
        }}
    }}

    @JvmStatic fun share(a: Activity, text: String) {{
        val i = android.content.Intent(android.content.Intent.ACTION_SEND)
        i.type = "text/plain"
        i.putExtra(android.content.Intent.EXTRA_TEXT, text)
        a.startActivity(android.content.Intent.createChooser(i, null))
    }}

    @JvmStatic fun hideKeyboard(a: Activity) {{
        val v = a.currentFocus ?: return
        (a.getSystemService(Context.INPUT_METHOD_SERVICE) as android.view.inputmethod.InputMethodManager).hideSoftInputFromWindow(v.windowToken, 0)
        v.clearFocus()
    }}

    private val fonts = HashMap<String, android.graphics.Typeface?>()

    /** One of the app's fonts (assets/fonts) by name; "" for the phone's own. Bold stays as it was. */
    @JvmStatic fun setFont(v: android.widget.TextView, name: String) {{
        val face = if (name.isEmpty()) null else fonts.getOrPut(name) {{
            listOf("ttf", "otf").firstNotNullOfOrNull {{ ext ->
                try {{ android.graphics.Typeface.createFromAsset(v.context.assets, "fonts/$name.$ext") }} catch (e: RuntimeException) {{ null }}
            }}
        }}
        val bold = v.typeface?.isBold == true
        v.setTypeface(android.graphics.Typeface.create(face ?: android.graphics.Typeface.DEFAULT, if (bold) android.graphics.Typeface.BOLD else android.graphics.Typeface.NORMAL))
    }}

    /** The text of one of the app's data files (assets) by name; "" if there is none. */
    @JvmStatic fun assetText(c: Context, name: String): String {{
        for (ext in listOf("txt", "json", "csv", "xml", "html", "md")) {{
            try {{
                return c.assets.open("$name.$ext").use {{ String(it.readBytes(), Charsets.UTF_8) }}.removePrefix("\uFEFF")
            }} catch (e: java.io.IOException) {{
            }}
        }}
        return ""
    }}

    /** A file path (camera, phone) or web address rather than one of the app's own files. */
    @JvmStatic fun outside(name: String): Boolean =
        name.startsWith("/") || name.startsWith("http://") || name.startsWith("https://") || name.startsWith("file:") || name.startsWith("content:")

    /** One of the app's pictures (res/drawable) by name, a picture file or a
     *  web address (loaded off the main thread); "" for none. */
    @JvmStatic fun setPicture(v: android.widget.ImageView, name: String) {{
        v.tag = name
        if (outside(name)) {{
            v.setImageDrawable(null)
            val most = Math.max(v.resources.displayMetrics.widthPixels, v.resources.displayMetrics.heightPixels)
            Thread {{
                val b = try {{ bitmap(v.context, name, most) }} catch (e: Exception) {{ null }}
                v.post {{ if (v.tag == name) v.setImageBitmap(b) }}
            }}.start()
            return
        }}
        val id = if (name.isEmpty()) 0 else v.resources.getIdentifier(name, "drawable", v.context.packageName)
        if (id != 0) v.setImageResource(id) else v.setImageDrawable(null)
    }}

    /** A picture file or web address, made small enough for the screen and
     *  turned the way the camera held it. */
    private fun bitmap(c: Context, name: String, most: Int): android.graphics.Bitmap? {{
        val bytes = when {{
            name.startsWith("http") -> URL(name).openStream().use {{ it.readBytes() }}
            name.startsWith("content:") -> c.contentResolver.openInputStream(android.net.Uri.parse(name))?.use {{ it.readBytes() }} ?: return null
            else -> java.io.File(name.removePrefix("file://")).readBytes()
        }}
        val size = android.graphics.BitmapFactory.Options()
        size.inJustDecodeBounds = true
        android.graphics.BitmapFactory.decodeByteArray(bytes, 0, bytes.size, size)
        var sample = 1
        while (Math.max(size.outWidth, size.outHeight) / (sample * 2) >= most) sample *= 2
        val o = android.graphics.BitmapFactory.Options()
        o.inSampleSize = sample
        val b = android.graphics.BitmapFactory.decodeByteArray(bytes, 0, bytes.size, o) ?: return null
        if (android.os.Build.VERSION.SDK_INT < 24) return b
        val e = android.media.ExifInterface(java.io.ByteArrayInputStream(bytes))
        val turn = when (e.getAttributeInt(android.media.ExifInterface.TAG_ORIENTATION, 1)) {{
            android.media.ExifInterface.ORIENTATION_ROTATE_90 -> 90f
            android.media.ExifInterface.ORIENTATION_ROTATE_180 -> 180f
            android.media.ExifInterface.ORIENTATION_ROTATE_270 -> 270f
            else -> 0f
        }}
        if (turn == 0f) return b
        val m = android.graphics.Matrix()
        m.postRotate(turn)
        return android.graphics.Bitmap.createBitmap(b, 0, 0, b.width, b.height, m, true)
    }}

    @JvmStatic fun setBg(v: android.view.View, color: Int) {{
        val bg = v.background
        if (bg is android.graphics.drawable.GradientDrawable) bg.setColor(color) else v.setBackgroundColor(color)
    }}

    @JvmStatic fun setRadius(v: android.view.View, r: Double) {{
        shape(v).setCornerRadius(dp(v, Math.max(r, 0.0)))
        v.clipToOutline = r > 0
    }}

    @JvmStatic fun setBorder(v: android.view.View, width: Double, color: Int) {{
        shape(v).setStroke(Math.round(dp(v, Math.max(width, 0.0))), color)
    }}

    @JvmStatic fun setElevation(v: android.view.View, e: Double) {{
        v.stateListAnimator = null
        v.elevation = dp(v, Math.max(e, 0.0))
    }}

    @JvmStatic fun toast(context: Context, text: String) {{
        Toast.makeText(context, text, Toast.LENGTH_SHORT).show()
    }}

    @JvmStatic fun randomBetween(min: Int, max: Int): Int = min + rng.nextInt(max - min + 1)

    @JvmStatic fun dpToPx(context: Context, dp: Int): Float =
        TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, dp.toFloat(), context.resources.displayMetrics)

    @JvmStatic fun screenWidth(context: Context): Int = context.resources.displayMetrics.widthPixels

    @JvmStatic fun screenHeight(context: Context): Int = context.resources.displayMetrics.heightPixels

    /** A web request off the main thread; [done] runs back on it with
     *  (answered with a 2xx code, the code or 0, the answer or what went wrong). */
    @JvmStatic fun web(activity: Activity, client: WebClient, method: String, url: String, body: String, done: (Boolean, Double, String) -> Unit) {{
        val headers = HashMap(client.headers)
        Thread {{
            var ok = false
            var code = 0
            var text: String
            try {{
                val c = URL(url.trim()).openConnection() as HttpURLConnection
                c.requestMethod = method
                c.connectTimeout = 15000
                c.readTimeout = 30000
                for ((k, v) in headers) c.setRequestProperty(k, v)
                if (body.isNotEmpty() && method != "GET") {{
                    if (headers.keys.none {{ it.equals("Content-Type", true) }}) c.setRequestProperty("Content-Type", "application/json")
                    c.doOutput = true
                    c.outputStream.use {{ it.write(body.toByteArray()) }}
                }}
                code = c.responseCode
                ok = code in 200..299
                val stream = if (ok) c.inputStream else c.errorStream
                text = stream?.bufferedReader()?.use {{ it.readText() }} ?: ""
                c.disconnect()
            }} catch (e: Exception) {{
                text = e.message ?: e.toString()
            }}
            activity.runOnUiThread {{ done(ok, code.toDouble(), text) }}
        }}.start()
    }}

    /** JSON values as the text blocks show them (nested ones stay JSON). */
    private fun jsonText(v: Any?): String = when (v) {{
        null, JSONObject.NULL -> ""
        is Double -> if (v == Math.floor(v) && !v.isInfinite()) v.toLong().toString() else v.toString()
        else -> v.toString()
    }}

    private fun objectToMap(o: JSONObject): HashMap<String, Any> {{
        val out = HashMap<String, Any>()
        for (k in o.keys()) out[k] = jsonText(o.opt(k))
        return out
    }}

    @JvmStatic fun jsonToMap(text: String): HashMap<String, Any> =
        try {{ objectToMap(JSONObject(text)) }} catch (e: Exception) {{ HashMap() }}

    @JvmStatic fun jsonToListMap(text: String): ArrayList<HashMap<String, Any>> {{
        val out = ArrayList<HashMap<String, Any>>()
        try {{
            val t = text.trim()
            val a = if (t.startsWith("[")) JSONArray(t) else JSONArray().put(JSONObject(t))
            for (i in 0 until a.length()) {{
                val item = a.opt(i)
                out.add(if (item is JSONObject) objectToMap(item) else hashMapOf("value" to jsonText(item)))
            }}
        }} catch (e: Exception) {{
        }}
        return out
    }}

    /** Keys in order, so the same map always reads the same. */
    private fun sorted(m: Map<*, *>): JSONObject = JSONObject(java.util.TreeMap(m.mapKeys {{ it.key.toString() }}))

    @JvmStatic fun toJson(v: Any): String = when (v) {{
        is Map<*, *> -> sorted(v).toString()
        is Collection<*> -> JSONArray(v.map {{ if (it is Map<*, *>) sorted(it) else it }}).toString()
        else -> v.toString()
    }}.replace("\\/", "/")
}}
"#,
        p.package
    ) + &supa_kotlin(p)
        + if p.screens.iter().any(|s| p.sheet_of(s) != Sheet::No) { SHEET_DRAG } else { "" }
}

/// Dragging a bottom sheet down closes it, as on iPhone: the sheet follows
/// the finger, closes past 30% of its height or on a flick down, and goes
/// back up otherwise. A list or scroll view inside scrolls back to its top
/// before the sheet moves.
const SHEET_DRAG: &str = r#"
/** Lets a bottom sheet screen be dragged down to close it. */
class SheetDrag(private val activity: Activity) {
    private val slop = android.view.ViewConfiguration.get(activity).scaledTouchSlop
    private val flick = 1000 * activity.resources.displayMetrics.density
    private val dim = 0.32f
    private var downX = 0f
    private var downY = 0f
    private var canDrag = false
    private var dragging = false
    private var tracker: android.view.VelocityTracker? = null
    private val sheet get() = activity.window.decorView

    /** From the activity's dispatchTouchEvent; `pass` hands the event on. */
    fun dispatch(ev: android.view.MotionEvent, pass: (android.view.MotionEvent) -> Boolean): Boolean {
        val raw = android.view.MotionEvent.obtain(ev).apply { setLocation(ev.rawX, ev.rawY) }
        when (ev.actionMasked) {
            android.view.MotionEvent.ACTION_DOWN -> {
                downX = ev.rawX
                downY = ev.rawY
                dragging = false
                val at = IntArray(2).also { sheet.getLocationOnScreen(it) }
                canDrag = ev.rawY >= at[1] && !scrollsUp(activity.findViewById(android.R.id.content), ev.rawX, ev.rawY)
                tracker?.recycle()
                tracker = android.view.VelocityTracker.obtain()
            }
            android.view.MotionEvent.ACTION_MOVE -> {
                val dy = ev.rawY - downY
                if (!dragging && canDrag && dy > slop && dy > Math.abs(ev.rawX - downX)) {
                    dragging = true
                    // the button or field under the finger lets go
                    val cancel = android.view.MotionEvent.obtain(ev).apply { action = android.view.MotionEvent.ACTION_CANCEL }
                    pass(cancel)
                    cancel.recycle()
                }
                if (dragging) move(Math.max(0f, dy - slop))
            }
        }
        tracker?.addMovement(raw)
        raw.recycle()
        if (!dragging) return pass(ev)
        val end = ev.actionMasked == android.view.MotionEvent.ACTION_UP || ev.actionMasked == android.view.MotionEvent.ACTION_CANCEL
        if (end) {
            dragging = false
            val t = tracker
            t?.computeCurrentVelocity(1000)
            val speed = t?.yVelocity ?: 0f
            t?.recycle()
            tracker = null
            if (ev.actionMasked == android.view.MotionEvent.ACTION_UP && (sheet.translationY > sheet.height * 0.3f || speed > flick)) close() else back()
        }
        return true
    }

    private fun move(y: Float) {
        sheet.translationY = y
        activity.window.setDimAmount(dim * (1f - y / Math.max(1, sheet.height)).coerceIn(0f, 1f))
    }

    private fun back() {
        sheet.animate().translationY(0f).setDuration(180).setUpdateListener { move(sheet.translationY) }.start()
    }

    private fun close() {
        sheet.animate().translationY(sheet.height.toFloat()).setDuration(160).setUpdateListener { move(sheet.translationY) }.withEndAction {
            // already off the screen: no closing slide
            activity.window.setWindowAnimations(0)
            activity.finish()
        }.start()
    }

    /** Whether a view under the point can still scroll up. */
    private fun scrollsUp(v: android.view.View?, x: Float, y: Float): Boolean {
        if (v == null || v.visibility != android.view.View.VISIBLE) return false
        val at = IntArray(2).also { v.getLocationOnScreen(it) }
        if (x < at[0] || x > at[0] + v.width || y < at[1] || y > at[1] + v.height) return false
        if (v.canScrollVertically(-1)) return true
        if (v is android.view.ViewGroup) for (i in 0 until v.childCount) if (scrollsUp(v.getChildAt(i), x, y)) return true
        return false
    }
}
"#;

/// The Supabase helper, with the linked project's address and key.
fn supa_kotlin(p: &Project) -> String {
    let link = p.supabase.clone().unwrap_or_default();
    SUPA_KOTLIN
        .replace("%URL%", &crate::codegen::quote_for(&link.base(), Lang::Kotlin))
        .replace("%KEY%", &crate::codegen::quote_for(link.key.trim(), Lang::Kotlin))
}

const SUPA_KOTLIN: &str = r#"
/** The linked Supabase project, and the signed-in person kept on the device. */
object Supa {
    const val URL = %URL%
    const val KEY = %KEY%
    private const val NOT_LINKED = "Supabase isn't linked yet: add your project's URL and key in Blockwise"

    private fun prefs(c: Context) = c.getSharedPreferences("supabase", Context.MODE_PRIVATE)
    @JvmStatic fun signedIn(c: Context): Boolean = prefs(c).getString("access", "")!!.isNotEmpty()
    @JvmStatic fun email(c: Context): String = prefs(c).getString("email", "")!!
    @JvmStatic fun userId(c: Context): String = prefs(c).getString("id", "")!!

    /** Keeps the session from a sign-in, sign-up or refresh answer. */
    private fun keep(c: Context, text: String) {
        try {
            val o = JSONObject(text)
            val access = o.optString("access_token")
            if (access.isEmpty()) return
            val user = o.optJSONObject("user")
            prefs(c).edit()
                .putString("access", access)
                .putString("refresh", o.optString("refresh_token"))
                .putString("email", user?.optString("email") ?: "")
                .putString("id", user?.optString("id") ?: "")
                .apply()
        } catch (e: Exception) {
        }
    }

    private fun client(c: Context, signedIn: Boolean): WebClient {
        val w = WebClient()
        w.headers["apikey"] = KEY
        val access = prefs(c).getString("access", "")!!
        w.headers["Authorization"] = "Bearer " + if (signedIn && access.isNotEmpty()) access else KEY
        return w
    }

    @JvmStatic fun auth(a: Activity, path: String, email: String, password: String, done: (Boolean, Double, String) -> Unit) {
        if (URL.isEmpty()) return done(false, 0.0, NOT_LINKED)
        val body = JSONObject().put("email", email.trim()).put("password", password).toString()
        AppUtil.web(a, client(a, false), "POST", "$URL/auth/v1/$path", body) { ok, code, text ->
            if (ok) keep(a, text)
            done(ok, code, text)
        }
    }

    @JvmStatic fun signOut(a: Activity) {
        if (URL.isNotEmpty() && signedIn(a)) AppUtil.web(a, client(a, true), "POST", "$URL/auth/v1/logout", "") { _, _, _ -> }
        prefs(a).edit().clear().apply()
    }

    private fun enc(s: String): String = java.net.URLEncoder.encode(s, "UTF-8").replace("+", "%20")

    /** A table request; rows come back as a JSON list. */
    @JvmStatic fun rest(a: Activity, method: String, table: String, column: String, value: String, body: String, done: (Boolean, Double, String) -> Unit) {
        if (URL.isEmpty()) return done(false, 0.0, NOT_LINKED)
        var url = "$URL/rest/v1/" + enc(table.trim())
        val query = ArrayList<String>()
        if (method == "GET") query.add("select=*")
        if (column.trim().isNotEmpty()) query.add(enc(column.trim()) + "=eq." + enc(value))
        if (query.isNotEmpty()) url += "?" + query.joinToString("&")
        send(a, method, url, body, true, done)
    }

    /** Sends, and once the sign-in has expired, renews it and sends again. */
    private fun send(a: Activity, method: String, url: String, body: String, retry: Boolean, done: (Boolean, Double, String) -> Unit) {
        val c = client(a, true)
        c.headers["Prefer"] = "return=representation"
        AppUtil.web(a, c, method, url, body) { ok, code, text ->
            val refresh = prefs(a).getString("refresh", "")!!
            if (code == 401.0 && retry && refresh.isNotEmpty()) {
                val r = JSONObject().put("refresh_token", refresh).toString()
                AppUtil.web(a, client(a, false), "POST", "$URL/auth/v1/token?grant_type=refresh_token", r) { rok, _, rtext ->
                    if (rok) {
                        keep(a, rtext)
                        send(a, method, url, body, false, done)
                    } else {
                        prefs(a).edit().clear().apply()
                        done(ok, code, text)
                    }
                }
            } else {
                done(ok, code, text)
            }
        }
    }

    /** The readable part of an error answer. */
    @JvmStatic fun error(text: String): String {
        try {
            val o = JSONObject(text)
            for (k in arrayOf("msg", "error_description", "message", "error")) {
                val v = o.optString(k)
                if (v.isNotEmpty()) return v
            }
        } catch (e: Exception) {
        }
        return text
    }
}
"#;

/// The Supabase helper for iPhone.
fn supa_swift(p: &Project) -> String {
    let link = p.supabase.clone().unwrap_or_default();
    SUPA_SWIFT
        .replace("%URL%", &crate::codegen::quote_for(&link.base(), Lang::Swift))
        .replace("%KEY%", &crate::codegen::quote_for(link.key.trim(), Lang::Swift))
}

const SUPA_SWIFT: &str = r#"
/// The linked Supabase project, and the signed-in person kept on the device.
enum Supa {
    static let url = %URL%
    static let key = %KEY%
    static let notLinked = "Supabase isn't linked yet: add your project's URL and key in Blockwise"
    private static var d: UserDefaults { UserDefaults.standard }
    static var signedIn: Bool { !(d.string(forKey: "supabase.access") ?? "").isEmpty }
    static var email: String { d.string(forKey: "supabase.email") ?? "" }
    static var userId: String { d.string(forKey: "supabase.id") ?? "" }

    /// Keeps the session from a sign-in, sign-up or refresh answer.
    private static func keep(_ text: String) {
        guard let data = text.data(using: .utf8),
              let o = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
              let access = o["access_token"] as? String, !access.isEmpty else { return }
        let user = o["user"] as? [String: Any] ?? [:]
        d.set(access, forKey: "supabase.access")
        d.set(o["refresh_token"] as? String ?? "", forKey: "supabase.refresh")
        d.set(user["email"] as? String ?? "", forKey: "supabase.email")
        d.set(user["id"] as? String ?? "", forKey: "supabase.id")
    }

    private static func forget() {
        for k in ["access", "refresh", "email", "id"] { d.removeObject(forKey: "supabase." + k) }
    }

    private static func client(_ signedIn: Bool) -> WebClient {
        let c = WebClient()
        c.headers["apikey"] = key
        let access = d.string(forKey: "supabase.access") ?? ""
        c.headers["Authorization"] = "Bearer " + (signedIn && !access.isEmpty ? access : key)
        return c
    }

    static func auth(_ path: String, _ email: String, _ password: String, _ done: @escaping (Bool, Double, String) -> Void) {
        if url.isEmpty { return done(false, 0, notLinked) }
        let body = Helpers.toJson(["email": email.trimmingCharacters(in: .whitespaces), "password": password])
        Helpers.web(client(false), "POST", url + "/auth/v1/" + path, body) { ok, code, text in
            if ok { keep(text) }
            done(ok, code, text)
        }
    }

    static func signOut() {
        if !url.isEmpty && signedIn { Helpers.web(client(true), "POST", url + "/auth/v1/logout", "") { _, _, _ in } }
        forget()
    }

    private static func enc(_ s: String) -> String {
        let ok = CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~")
        return s.addingPercentEncoding(withAllowedCharacters: ok) ?? s
    }

    /// A table request; rows come back as a JSON list.
    static func rest(_ method: String, _ table: String, _ column: String, _ value: String, _ body: String, _ done: @escaping (Bool, Double, String) -> Void) {
        if url.isEmpty { return done(false, 0, notLinked) }
        var u = url + "/rest/v1/" + enc(table.trimmingCharacters(in: .whitespaces))
        var query: [String] = []
        if method == "GET" { query.append("select=*") }
        let col = column.trimmingCharacters(in: .whitespaces)
        if !col.isEmpty { query.append(enc(col) + "=eq." + enc(value)) }
        if !query.isEmpty { u += "?" + query.joined(separator: "&") }
        send(method, u, body, true, done)
    }

    /// Sends, and once the sign-in has expired, renews it and sends again.
    private static func send(_ method: String, _ u: String, _ body: String, _ retry: Bool, _ done: @escaping (Bool, Double, String) -> Void) {
        let c = client(true)
        c.headers["Prefer"] = "return=representation"
        Helpers.web(c, method, u, body) { ok, code, text in
            let refresh = d.string(forKey: "supabase.refresh") ?? ""
            if code == 401 && retry && !refresh.isEmpty {
                Helpers.web(client(false), "POST", url + "/auth/v1/token?grant_type=refresh_token", Helpers.toJson(["refresh_token": refresh])) { rok, _, rtext in
                    if rok {
                        keep(rtext)
                        send(method, u, body, false, done)
                    } else {
                        forget()
                        done(ok, code, text)
                    }
                }
            } else {
                done(ok, code, text)
            }
        }
    }

    /// The readable part of an error answer.
    static func error(_ text: String) -> String {
        if let data = text.data(using: .utf8), let o = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] {
            for k in ["msg", "error_description", "message", "error"] {
                if let v = o[k] as? String, !v.isEmpty { return v }
            }
        }
        return text
    }
}
"#;

/// What a crash restarts into: a dialog with the error and one button
/// (title, button and the "error" extra are in runtime.rs).
pub fn kotlin_crash_activity(p: &Project) -> String {
    format!(
        "package {pkg}\n\nimport android.app.Activity\nimport android.app.AlertDialog\nimport android.os.Bundle\nimport android.widget.TextView\n\nclass CrashActivity : Activity() {{\n\n    override fun onCreate(savedInstanceState: Bundle?) {{\n        super.onCreate(savedInstanceState)\n        val error = intent?.getStringExtra(\"{extra}\") ?: return\n        val dialog = AlertDialog.Builder(this)\n            .setTitle(\"{title}\")\n            .setMessage(error)\n            .setPositiveButton(\"{button}\") {{ _, _ -> finish() }}\n            .create()\n        dialog.show()\n        // let the person select and copy the error\n        dialog.findViewById<TextView>(android.R.id.message)?.setTextIsSelectable(true)\n    }}\n}}\n",
        pkg = p.package,
        extra = crate::runtime::CRASH_EXTRA,
        title = crate::runtime::CRASH_TITLE,
        button = crate::runtime::CRASH_BUTTON,
    )
}

/// Catches a crash anywhere in the app and reopens it on CrashActivity.
pub fn kotlin_crash_catcher(p: &Project) -> String {
    // The crash screen runs in a process of its own (see the manifest), so it
    // is started straight away and outlives this one. The task is cleared
    // first: otherwise Android reopens the screen that crashed, which, when
    // it crashed while opening, crashes again and again.
    format!(
        "package {pkg}\n\nimport android.app.Application\nimport android.content.Intent\nimport android.os.Process\nimport android.util.Log\nimport kotlin.system.exitProcess\n\nclass CrashCatcher : Application() {{\n\n    override fun onCreate() {{\n        super.onCreate()\n        Thread.setDefaultUncaughtExceptionHandler {{ thread, error ->\n            Log.e(\"{tag}\", \"crash: \" + error + \" on thread \" + thread.name, error)\n            try {{\n                val intent = Intent(applicationContext, CrashActivity::class.java)\n                intent.flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK\n                intent.putExtra(\"{extra}\", Log.getStackTraceString(error))\n                startActivity(intent)\n            }} catch (e: Throwable) {{\n                Log.e(\"{tag}\", \"couldn't open the crash screen\", e)\n            }}\n            Process.killProcess(Process.myPid())\n            exitProcess(10)\n        }}\n    }}\n}}\n",
        pkg = p.package,
        tag = crate::codegen::LOG_TAG,
        extra = crate::runtime::CRASH_EXTRA,
    )
}

/// Plain http only to this computer (a local Supabase, a test server):
/// localhost on a phone, 10.0.2.2 from the Android emulator.
const NETWORK_SECURITY: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<network-security-config>\n\t<domain-config cleartextTrafficPermitted=\"true\">\n\t\t<domain includeSubdomains=\"false\">localhost</domain>\n\t\t<domain includeSubdomains=\"false\">127.0.0.1</domain>\n\t\t<domain includeSubdomains=\"false\">10.0.2.2</domain>\n\t</domain-config>\n</network-security-config>\n";

/// The app's themes: light screens, the app bar and status bar in the theme
/// colour, its title and arrow white (Blockwise), dark on a light bar
/// (Blockwise.LightBar), no app bar (Blockwise.NoBar), or a bottom sheet
/// sliding up over the screen below (Blockwise.Sheet). The navigation bar is
/// white, as in the preview. Buttons keep
/// their text as typed, as in the preview.
/// A bottom sheet's background: white with round top corners and the handle.
const SHEET_BACKGROUND: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<layer-list xmlns:android=\"http://schemas.android.com/apk/res/android\">\n\t<item>\n\t\t<shape android:shape=\"rectangle\">\n\t\t\t<solid android:color=\"#FFFFFFFF\"/>\n\t\t\t<corners android:topLeftRadius=\"24dp\" android:topRightRadius=\"24dp\"/>\n\t\t</shape>\n\t</item>\n\t<item android:top=\"8dp\" android:width=\"32dp\" android:height=\"4dp\" android:gravity=\"top|center_horizontal\">\n\t\t<shape android:shape=\"rectangle\">\n\t\t\t<solid android:color=\"#FF79747E\"/>\n\t\t\t<corners android:radius=\"2dp\"/>\n\t\t</shape>\n\t</item>\n</layer-list>\n";
const SHEET_IN: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<translate xmlns:android=\"http://schemas.android.com/apk/res/android\" android:fromYDelta=\"100%\" android:toYDelta=\"0\" android:duration=\"250\" android:interpolator=\"@android:interpolator/decelerate_cubic\"/>\n";
const SHEET_OUT: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<translate xmlns:android=\"http://schemas.android.com/apk/res/android\" android:fromYDelta=\"0\" android:toYDelta=\"100%\" android:duration=\"200\" android:interpolator=\"@android:interpolator/accelerate_cubic\"/>\n";

const ANDROID_THEMES: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<resources xmlns:tools=\"http://schemas.android.com/tools\">\n\t<style name=\"Blockwise\" parent=\"@android:style/Theme.Material.Light.DarkActionBar\">\n\t\t<item name=\"android:colorPrimary\">#5B5BD6</item>\n\t\t<item name=\"android:colorPrimaryDark\">#4A4AC4</item>\n\t\t<item name=\"android:colorAccent\">#5B5BD6</item>\n\t\t<item name=\"android:buttonStyle\">@style/Blockwise.Button</item>\n\t\t<item name=\"android:navigationBarColor\">#FFFFFFFF</item>\n\t\t<item name=\"android:windowLightNavigationBar\" tools:targetApi=\"27\">true</item>\n\t</style>\n\t<style name=\"Blockwise.LightBar\" parent=\"@android:style/Theme.Material.Light\">\n\t\t<item name=\"android:colorPrimary\">#5B5BD6</item>\n\t\t<item name=\"android:colorPrimaryDark\">#4A4AC4</item>\n\t\t<item name=\"android:colorAccent\">#5B5BD6</item>\n\t\t<item name=\"android:buttonStyle\">@style/Blockwise.Button</item>\n\t\t<item name=\"android:navigationBarColor\">#FFFFFFFF</item>\n\t\t<item name=\"android:windowLightNavigationBar\" tools:targetApi=\"27\">true</item>\n\t</style>\n\t<style name=\"Blockwise.NoBar\" parent=\"@android:style/Theme.Material.Light.NoActionBar\">\n\t\t<item name=\"android:colorPrimary\">#5B5BD6</item>\n\t\t<item name=\"android:colorPrimaryDark\">#4A4AC4</item>\n\t\t<item name=\"android:colorAccent\">#5B5BD6</item>\n\t\t<item name=\"android:buttonStyle\">@style/Blockwise.Button</item>\n\t\t<item name=\"android:navigationBarColor\">#FFFFFFFF</item>\n\t\t<item name=\"android:windowLightNavigationBar\" tools:targetApi=\"27\">true</item>\n\t</style>\n\t<style name=\"Blockwise.Sheet\" parent=\"@android:style/Theme.Material.Light.Dialog.NoActionBar\">\n\t\t<item name=\"android:colorPrimary\">#5B5BD6</item>\n\t\t<item name=\"android:colorAccent\">#5B5BD6</item>\n\t\t<item name=\"android:buttonStyle\">@style/Blockwise.Button</item>\n\t\t<item name=\"android:windowBackground\">@drawable/bw_sheet</item>\n\t\t<item name=\"android:windowMinWidthMajor\">100%</item>\n\t\t<item name=\"android:windowMinWidthMinor\">100%</item>\n\t\t<item name=\"android:backgroundDimAmount\">0.32</item>\n\t\t<item name=\"android:windowAnimationStyle\">@style/Blockwise.SheetSlide</item>\n\t</style>\n\t<style name=\"Blockwise.SheetSlide\">\n\t\t<item name=\"android:windowEnterAnimation\">@anim/bw_sheet_in</item>\n\t\t<item name=\"android:windowExitAnimation\">@anim/bw_sheet_out</item>\n\t</style>\n\t<style name=\"Blockwise.Button\" parent=\"@android:style/Widget.Material.Button\">\n\t\t<item name=\"android:textAllCaps\">false</item>\n\t</style>\n</resources>\n";

/// The drawable an app bar icon is written to (res/drawable).
pub fn android_icon_name(icon: &str) -> String {
    let known = BAR_ICONS.iter().any(|(n, _, _)| *n == icon);
    format!("bw_icon_{}", if known { icon } else { "more" })
}

/// Which of ANDROID_THEMES a screen uses.
fn android_theme(bar: &AppBar) -> &'static str {
    if bar.hidden {
        "Blockwise.NoBar"
    } else if is_light(bar.title(false)) {
        "Blockwise"
    } else {
        "Blockwise.LightBar"
    }
}

/// onCreate's first lines: the colours picked for the app bar and the
/// status bar, and the up arrow on a screen opened from another.
fn kotlin_app_bar(bar: &AppBar) -> String {
    let mut out = String::new();
    if bar.color != 0 {
        out.push_str(&format!("window.statusBarColor = 0x{:08X}.toInt()\n", bar.status()));
        if is_light(bar.status()) {
            out.push_str("if (android.os.Build.VERSION.SDK_INT >= 23) window.decorView.systemUiVisibility = android.view.View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR\n");
        }
        if !bar.hidden {
            out.push_str(&format!("actionBar?.setBackgroundDrawable(android.graphics.drawable.ColorDrawable(0x{:08X}.toInt()))\n", bar.bar(false)));
        }
    }
    if !bar.hidden && !bar.no_back {
        out.push_str("if (!isTaskRoot) actionBar?.setDisplayHomeAsUpEnabled(true)\n");
        if bar.title_color != 0 {
            // the theme's arrow, in the title's colour
            out.push_str(&format!("theme.obtainStyledAttributes(intArrayOf(android.R.attr.homeAsUpIndicator)).apply {{\ngetDrawable(0)?.mutate()?.let {{ it.setTint(0x{:08X}.toInt()); actionBar?.setHomeAsUpIndicator(it) }}\nrecycle()\n}}\n", bar.title(false)));
        }
    }
    out
}

/// The keyboard stays hidden when a screen opens or comes back (after a
/// sheet or another screen closes) until a text field is tapped.
const KEYS: &str = " android:windowSoftInputMode=\"stateAlwaysHidden\"";

/// App settings for the Kotlin project: name, icon, screens, and the first
/// screen marked as the one the launcher opens.
pub fn kotlin_manifest(p: &Project) -> String {
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\">\n\t<uses-permission android:name=\"android.permission.INTERNET\"/>\n\t<uses-permission android:name=\"android.permission.VIBRATE\"/>\n\t<application\n\t\tandroid:name=\".CrashCatcher\"\n\t\tandroid:label=\"{}\"\n\t\tandroid:icon=\"@mipmap/app_icon\"\n\t\tandroid:roundIcon=\"@mipmap/app_icon\"\n\t\tandroid:networkSecurityConfig=\"@xml/network_security_config\"\n\t\tandroid:theme=\"@style/Blockwise\">\n",
        crate::codegen::xml_escape(&p.app_name)
    );
    for (i, s) in p.screens.iter().enumerate() {
        let theme = match if p.sheet_of(s) != Sheet::No { "Blockwise.Sheet" } else { android_theme(&s.look()) } {
            "Blockwise" => String::new(),
            t => format!(" android:theme=\"@style/{t}\""),
        };
        if i == 0 {
            out.push_str(&format!("\t\t<activity android:name=\".{}\"{theme}{KEYS} android:exported=\"true\">\n\t\t\t<intent-filter>\n\t\t\t\t<action android:name=\"android.intent.action.MAIN\"/>\n\t\t\t\t<category android:name=\"android.intent.category.LAUNCHER\"/>\n\t\t\t</intent-filter>\n\t\t</activity>\n", s.activity_name()));
        } else {
            out.push_str(&format!("\t\t<activity android:name=\".{}\"{theme}{KEYS}/>\n", s.activity_name()));
        }
    }
    // where the camera app writes the photo or video it takes (AppMedia)
    out.push_str(&format!("\t\t<provider android:name=\".MediaFiles\" android:authorities=\"{}.media\" android:exported=\"false\" android:grantUriPermissions=\"true\"/>\n", p.package));
    out.push_str("\t\t<activity android:name=\".CrashActivity\" android:process=\":crash\"/>\n\t</application>\n</manifest>\n");
    out
}

/// Build tool versions written into the Gradle files.
#[derive(Clone, Debug)]
pub struct GradleSetup {
    pub android_plugin: String,
    pub kotlin_plugin: String,
    pub compile_sdk: u32,
    /// The SDK folder, when known (local.properties).
    pub sdk_dir: Option<std::path::PathBuf>,
    /// An aapt2 program from the SDK, used instead of downloading one.
    pub aapt2: Option<std::path::PathBuf>,
}

impl Default for GradleSetup {
    fn default() -> Self {
        // a pairing checked to build together
        GradleSetup { android_plugin: "8.13.2".into(), kotlin_plugin: "2.2.20".into(), compile_sdk: 35, sdk_dir: None, aapt2: None }
    }
}

/// The Gradle files that make the Kotlin folder a project Android Studio
/// (or the gradle command) can build: (path inside the project, text).
pub fn gradle_files(p: &Project, g: &GradleSetup) -> Vec<(String, String)> {
    let mut v = vec![
        (
            "settings.gradle.kts".to_string(),
            format!(
                "pluginManagement {{ repositories {{ google(); mavenCentral(); gradlePluginPortal() }} }}\ndependencyResolutionManagement {{ repositories {{ google(); mavenCentral() }} }}\nrootProject.name = {}\ninclude(\":app\")\n",
                crate::codegen::quote(&p.name)
            ),
        ),
        (
            "build.gradle.kts".to_string(),
            format!(
                "buildscript {{\n    repositories {{ google(); mavenCentral() }}\n    dependencies {{\n        classpath(\"com.android.tools.build:gradle:{}\")\n        classpath(\"org.jetbrains.kotlin:kotlin-gradle-plugin:{}\")\n    }}\n}}\n",
                g.android_plugin, g.kotlin_plugin
            ),
        ),
        (
            "app/build.gradle.kts".to_string(),
            format!(
                "plugins {{\n    id(\"com.android.application\")\n    id(\"org.jetbrains.kotlin.android\")\n}}\n\nandroid {{\n    namespace = \"{pkg}\"\n    compileSdk = {sdk}\n    defaultConfig {{\n        applicationId = \"{pkg}\"\n        minSdk = 21\n        targetSdk = {sdk}\n        versionCode = {code}\n        versionName = {name}\n    }}\n    compileOptions {{\n        sourceCompatibility = JavaVersion.VERSION_17\n        targetCompatibility = JavaVersion.VERSION_17\n    }}\n    kotlinOptions {{ jvmTarget = \"17\" }}\n}}\n",
                pkg = p.package,
                sdk = g.compile_sdk,
                code = p.ver_code.max(1),
                name = crate::codegen::quote(&p.ver_name)
            ),
        ),
    ];
    let mut props = String::from("org.gradle.jvmargs=-Xmx2g\nandroid.useAndroidX=false\nkotlin.code.style=official\n");
    if let Some(a) = &g.aapt2 {
        props.push_str(&format!("android.aapt2FromMavenOverride={}\n", a.display().to_string().replace('\\', "/")));
    }
    v.push(("gradle.properties".to_string(), props));
    if let Some(d) = &g.sdk_dir {
        v.push(("local.properties".to_string(), format!("sdk.dir={}\n", d.display().to_string().replace('\\', "/"))));
    }
    v
}

/// Write the Kotlin Android project. `trace` makes a debug build that logs
/// what the blocks do.
/// Material's "visibility" icons (Apache 2.0), for a password field's eye.
const EYE_PATH: &str = "M12,4.5C7,4.5 2.73,7.61 1,12c1.73,4.39 6,7.5 11,7.5s9.27,-3.11 11,-7.5c-1.73,-4.39 -6,-7.5 -11,-7.5zM12,17c-2.76,0 -5,-2.24 -5,-5s2.24,-5 5,-5 5,2.24 5,5 -2.24,5 -5,5zM12,9c-1.66,0 -3,1.34 -3,3s1.34,3 3,3 3,-1.34 3,-3 -1.34,-3 -3,-3z";
const EYE_OFF_PATH: &str = "M12,7c2.76,0 5,2.24 5,5 0,0.65 -0.13,1.26 -0.36,1.83l2.92,2.92c1.51,-1.26 2.7,-2.89 3.43,-4.75 -1.73,-4.39 -6,-7.5 -11,-7.5 -1.4,0 -2.74,0.25 -3.98,0.7l2.16,2.16C10.74,7.13 11.35,7 12,7zM2,4.27l2.28,2.28 0.46,0.46C3.08,8.3 1.78,10.02 1,12c1.73,4.39 6,7.5 11,7.5 1.55,0 3.03,-0.3 4.38,-0.84l0.42,0.42L19.73,22 21,20.73 3.27,3 2,4.27zM7.53,9.8l1.55,1.55c-0.05,0.21 -0.08,0.43 -0.08,0.65 0,1.66 1.34,3 3,3 0.22,0 0.44,-0.03 0.65,-0.08l1.55,1.55c-0.67,0.33 -1.41,0.53 -2.2,0.53 -2.76,0 -5,-2.24 -5,-5 0,-0.79 0.2,-1.53 0.53,-2.2zM11.84,9.02l3.15,3.15 0.02,-0.16c0,-1.66 -1.34,-3 -3,-3l-0.17,0.01z";

fn eye_drawable(path: &str) -> String {
    format!("<vector xmlns:android=\"http://schemas.android.com/apk/res/android\" android:width=\"22dp\" android:height=\"22dp\" android:viewportWidth=\"24\" android:viewportHeight=\"24\">\n    <path android:fillColor=\"#8A000000\" android:pathData=\"{path}\"/>\n</vector>\n")
}

pub fn write_kotlin_project(p: &Project, dir: &std::path::Path, g: &GradleSetup, trace: bool) -> std::io::Result<()> {
    let main = dir.join("app/src/main");
    let code = main.join("kotlin").join(p.package.replace('.', "/"));
    let res = main.join("res/layout");
    // a rebuild reuses the folder: drop sources from older builds (the Java
    // ones, a renamed package, deleted screens) so nothing is declared twice.
    // Leftover Java sources mean the compiler's caches are stale too: Kotlin's
    // incremental build drops the class it thinks was deleted, so start clean
    if main.join("java").exists() {
        let _ = std::fs::remove_dir_all(dir.join("app/build"));
    }
    for old in [main.join("java"), main.join("kotlin"), res.clone()] {
        if old.exists() {
            std::fs::remove_dir_all(&old)?;
        }
    }
    std::fs::create_dir_all(&code)?;
    std::fs::create_dir_all(&res)?;
    for s in &p.screens {
        std::fs::write(code.join(format!("{}.kt", s.activity_name())), kotlin_with(p, s, trace))?;
        std::fs::write(res.join(format!("{}.xml", s.name)), crate::codegen::xml(s))?;
    }
    std::fs::write(code.join("AppUtil.kt"), kotlin_util(p))?;
    std::fs::write(code.join("CrashActivity.kt"), kotlin_crash_activity(p))?;
    std::fs::write(code.join("CrashCatcher.kt"), kotlin_crash_catcher(p))?;
    std::fs::write(main.join("AndroidManifest.xml"), kotlin_manifest(p))?;
    std::fs::create_dir_all(main.join("res/xml"))?;
    std::fs::write(main.join("res/xml/network_security_config.xml"), NETWORK_SECURITY)?;
    std::fs::create_dir_all(main.join("res/values"))?;
    std::fs::write(main.join("res/values/themes.xml"), ANDROID_THEMES)?;
    // pictures in drawable (1 pixel is 1 dp), sounds and videos in raw, fonts and data
    // files in assets (read by their file name)
    for d in ["res/drawable", "res/raw", "assets"] {
        let _ = std::fs::remove_dir_all(main.join(d));
    }
    for (f, path) in p.file_paths() {
        let d = main.join(match f.kind {
            FileKind::Image => "res/drawable",
            FileKind::Sound | FileKind::Video => "res/raw",
            FileKind::Font => "assets/fonts",
            FileKind::Data => "assets",
        });
        std::fs::create_dir_all(&d)?;
        std::fs::copy(&path, d.join(f.file_name()))?;
    }
    // the eye of password fields
    std::fs::create_dir_all(main.join("res/drawable"))?;
    std::fs::write(main.join("res/drawable/blockwise_eye.xml"), eye_drawable(EYE_PATH))?;
    std::fs::write(main.join("res/drawable/blockwise_eye_off.xml"), eye_drawable(EYE_OFF_PATH))?;
    // bottom sheets: their background and how they slide
    std::fs::write(main.join("res/drawable/bw_sheet.xml"), SHEET_BACKGROUND)?;
    std::fs::create_dir_all(main.join("res/anim"))?;
    std::fs::write(main.join("res/anim/bw_sheet_in.xml"), SHEET_IN)?;
    std::fs::write(main.join("res/anim/bw_sheet_out.xml"), SHEET_OUT)?;
    // the app bars' icons
    let mut icons: Vec<&str> = p.screens.iter().flat_map(|s| s.app_bar.actions.iter().map(|a| a.icon.as_str())).collect();
    icons.sort();
    icons.dedup();
    for icon in icons {
        std::fs::write(main.join(format!("res/drawable/{}.xml", android_icon_name(icon))), crate::icons::android_vector(bar_icon(icon).0))?;
    }
    for (path, bytes) in crate::icons::android_icon_files(p) {
        let f = main.join("res").join(path);
        if let Some(parent) = f.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(f, bytes)?;
    }
    for (path, text) in gradle_files(p, g) {
        let f = dir.join(path);
        if let Some(parent) = f.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(f, text)?;
    }
    Ok(())
}

/// Write the Swift sources of the iPhone app into `dir`.
pub fn write_swift_sources(p: &Project, dir: &std::path::Path, trace: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for s in &p.screens {
        std::fs::write(dir.join(format!("{}Screen.swift", swift_name(s))), swift_screen_with(p, s, trace))?;
    }
    std::fs::write(dir.join("App.swift"), swift_app(p))?;
    std::fs::write(dir.join("Helpers.swift"), swift_helpers(p))?;
    // pictures, sounds, videos, fonts and data files go in the app as they are
    let res = dir.join("Resources");
    let _ = std::fs::remove_dir_all(&res);
    for (f, path) in p.file_paths() {
        std::fs::create_dir_all(&res)?;
        std::fs::copy(&path, res.join(f.file_name()))?;
    }
    Ok(())
}

/// Write the Kotlin Android project and/or the Swift iPhone sources under
/// `base` (android-kotlin and ios-swift folders).
pub fn export_native(p: &Project, base: &std::path::Path, kotlin: bool, swift: bool) -> std::io::Result<()> {
    if kotlin {
        write_kotlin_project(p, &base.join("android-kotlin"), &crate::device::gradle_setup(), false)?;
    }
    if swift {
        let src = base.join("ios-swift").join(sanitize(&p.name));
        write_swift_sources(p, &src, false)?;
        for (path, bytes) in crate::icons::ios_asset_files(p) {
            let f = src.join("Assets.xcassets").join(path);
            if let Some(parent) = f.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(f, bytes)?;
        }
        std::fs::write(
            base.join("ios-swift").join("HOW-TO-OPEN.txt"),
            "Make a new iOS App project in Xcode (SwiftUI, iOS 17 or later). Delete its ContentView and App files and its Assets.xcassets, then drag in the Swift files, the Assets.xcassets folder and what is in Resources (the pictures, sounds, videos, fonts and data files) from here. The app icon is in Assets.xcassets. If the app uses the camera, add \"Privacy - Camera Usage Description\" and \"Privacy - Microphone Usage Description\" to the target's Info (what the phone tells the person when it asks).\n",
        )?;
    }
    Ok(())
}
