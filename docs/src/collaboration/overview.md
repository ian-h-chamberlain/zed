# Collaboration

At Zed, we believe that great things are built by great people working together.
We have designed Zed to help individuals work faster and help teams of people work together more effectively.

In Zed, all collaboration happens in the collaboration panel, which can be opened via {#kb collab_panel::ToggleFocus} or `collab panel: toggle focus` from the command palette.
You will need to [sign in](../../authentication.md#signing-in) in order to access features within the collaboration panel.

## Channels vs Contacts

There are two ways to start a collaboration session in Zed:

- **Channels**: Ongoing rooms where team members can share projects, collaborate on code, and maintain ambient awareness of what everyone is working on.
  Channels support persistent notes, nested permissions, and public/guest access.
- **Contacts**: Your personal contact list for ad-hoc private collaboration sessions.
  Great for one-on-ones or quick calls with trusted collaborators.

Once you're in a call—whether through a channel or with contacts—the collaboration experience is largely the same.
The sections below cover these shared features.

## Voice Chat

When joining a call, Zed will automatically share your microphone if your OS allows it.
You can disable this behavior via the [`mute_on_join`](../configuring-zed.md#calls) setting.

## Sharing a Project

After joining a call, you can share a project with other participants by clicking the `Share` button next to the project name in the title bar.
This enables collaborators to edit code hosted on your machine as though they had it checked out locally.

You can remove a project from a call by clicking the `Unshare` button. Collaborators currently in that project will be disconnected and cannot rejoin unless you share it again.

## Working in a Shared Project

When you're editing someone else's project, you have the full power of the editor at your fingertips—jump to definitions, use the AI assistant, and see diagnostic errors.
This makes pairing powerful: one person can implement the current method while the other researches the solution to the next problem.
And because you're running your own config, it feels like you're on your own machine.

We aim to eliminate the distinction between local and remote projects as much as possible.
Collaborators can open, edit, and save files, perform searches, interact with the language server, and more.

### Collaborator Visibility

Your Zed window shows call participants in the title bar:

![A new Zed call with two collaborators](https://zed.dev/img/collaboration/new-call.png)

- Collaborators in the same project as you appear in color with a cursor color
- Collaborators in other projects appear in gray
- If a collaborator is in an unshared project, you cannot follow them until they share it or return to a shared project

## Following a Collaborator

To follow a collaborator, click on their avatar in the top right of the window.
You can also cycle through collaborators using `workspace: follow next collaborator` ({#kb workspace::FollowNextCollaborator}).

When you join a project, you'll immediately start following the collaborator who invited you.

![Automatically following the person inviting us to a project](https://zed.dev/img/collaboration/joining-a-call.png)

While following a collaborator, you will:

- Follow their cursor and scroll position
- Follow them to other files in the same project
- Instantly swap to viewing their screen share if they leave the project

If you move your cursor or make an edit in that pane, you'll stop following.
Click their avatar or press {#kb workspace::FollowNextCollaborator} to resume.

### How Following Works

Following is confined to a particular pane.
When a pane is following a collaborator, it is outlined in their cursor color.

This pane-specific behavior allows you to follow someone in one pane while navigating independently in another—an effective layout for some collaboration styles.

## Screen Sharing

Share your screen with collaborators by clicking the `Share screen` button in the top right of the window.

Collaborators will see your screen if they are following you and you start viewing a window outside Zed or a project that is not shared.

Call participants can open a dedicated tab for your screen share by opening the contacts menu in the top right and clicking on the `Screen` entry.

> **Note**: Collaborators can see your entire screen when you are screen sharing, so be careful not to share anything sensitive.
> Remember to stop screen sharing when you are finished.

### Terminal Collaboration

You can follow what a collaborator is doing in their terminal by having them share their screen and following it.

In the future, we plan to allow direct terminal collaboration within shared projects.

## Leaving a Call

You can leave a call by opening the contacts menu in the top right and clicking on the `Leave call` button.

---

## Channel-Specific Features

These features are unique to channels:

### Channel Notes

Each channel has a notes file to track current status, ideas, or design discussions before diving into code.
This is similar to a Google Doc, powered by Zed's collaborative editing and persisted to our servers.

### Public Channels and Guests

Channels can be made public, allowing anyone with the link to join.

Guest users can hear and see everything happening in the channel and have read-only access to projects and channel notes.
To allow a guest to participate during a call, right-click on them in the Collaboration Panel and select "Allow Write Access"—this lets them edit shared projects, use their microphone, and share their screen.

### Managing Members and Permissions

By default, channels you create are private.
Invite collaborators by right-clicking a channel and selecting `Manage members`.

Permissions are inherited through nested channels.
For example, adding someone to a `#zed` channel automatically grants them access to child channels like `#core-editor` or `#new-languages`.

---

## Adding Contacts

1. In the collaboration panel, click the `+` button next to the `Contacts` section
2. Search for the contact using their GitHub handle
   _Note: The contact must be an existing Zed user who has completed GitHub authentication._
3. Your contact will receive a notification.
Once they accept, you'll both appear in each other's contact list.

---

> **Warning**: Only collaborate with people you trust.
> Sharing a project gives collaborators access to your local file system.
> They could potentially access files beyond the shared project.
>
> In the future, we will add more safeguards and controls over what collaborators can do.
> For now, only collaborate with people you trust.
